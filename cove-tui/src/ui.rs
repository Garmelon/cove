mod cove;
mod input;
mod layout;
mod overlays;
mod pane;
mod rooms;
mod styles;
mod textline;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io::Stdout;

use crossterm::event::{
    Event as CEvent, EventStream, KeyCode, KeyEvent, MouseEvent, MouseEventKind,
};
use futures::StreamExt;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::{Frame, Terminal};

use crate::backend::cove::conn::Event as CoveEvent;
use crate::backend::cove::room::CoveRoom;
use crate::backend::Event as BEvent;
use crate::config::Config;
use crate::ui::overlays::OverlayReaction;

use self::cove::CoveUi;
use self::input::EventHandler;
use self::overlays::{Overlay, SwitchRoom, SwitchRoomState};
use self::pane::PaneInfo;
use self::rooms::Rooms;

pub type Backend = CrosstermBackend<Stdout>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RoomId {
    Cove(String),
}

#[derive(Debug)]
pub enum UiEvent {
    Term(CEvent),
    Room(BEvent),
    Redraw,
}

impl From<BEvent> for UiEvent {
    fn from(event: BEvent) -> Self {
        Self::Room(event)
    }
}

enum EventHandleResult {
    Continue,
    Stop,
}

pub struct Ui {
    config: &'static Config,
    event_tx: UnboundedSender<UiEvent>,

    cove_rooms: HashMap<String, CoveUi>,
    room: Option<RoomId>,

    rooms_pane: PaneInfo,
    users_pane: PaneInfo,

    overlay: Option<Overlay>,

    last_area: Rect,
}

impl Ui {
    fn new(config: &'static Config, event_tx: UnboundedSender<UiEvent>) -> Self {
        Self {
            config,
            event_tx,

            cove_rooms: HashMap::new(),
            room: None,

            rooms_pane: PaneInfo::default(),
            users_pane: PaneInfo::default(),

            overlay: None,

            last_area: Rect::default(),
        }
    }

    pub async fn run(
        config: &'static Config,
        terminal: &mut Terminal<Backend>,
    ) -> anyhow::Result<()> {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mut ui = Self::new(config, event_tx.clone());

        tokio::select! {
            e = ui.run_main(terminal, &mut event_rx) => e,
            e = Self::shovel_crossterm_events(event_tx) => e,
        }
    }

    async fn shovel_crossterm_events(tx: UnboundedSender<UiEvent>) -> anyhow::Result<()> {
        // Implemented manually because UnboundedSender doesn't implement the Sink trait
        let mut stream = EventStream::new();
        while let Some(event) = stream.next().await {
            tx.send(UiEvent::Term(event?))?;
        }
        Ok(())
    }

    async fn run_main(
        &mut self,
        terminal: &mut Terminal<Backend>,
        event_rx: &mut UnboundedReceiver<UiEvent>,
    ) -> anyhow::Result<()> {
        loop {
            // 1. Render current state
            terminal.autoresize()?;

            let mut frame = terminal.get_frame();
            self.last_area = frame.size();
            self.render(&mut frame).await?;

            // Do a little dance to please the borrow checker
            let cursor = frame.cursor();
            drop(frame);

            terminal.flush()?;
            terminal.set_cursor_opt(cursor)?; // Must happen after flush
            terminal.flush_backend()?;
            terminal.swap_buffers();

            // 2. Handle events (in batches)
            let mut event = match event_rx.recv().await {
                Some(event) => event,
                None => return Ok(()),
            };
            loop {
                let result = match event {
                    UiEvent::Term(CEvent::Key(event)) => self.handle_key_event(event).await?,
                    UiEvent::Term(CEvent::Mouse(event)) => self.handle_mouse_event(event).await?,
                    UiEvent::Term(CEvent::Resize(_, _)) => EventHandleResult::Continue,
                    UiEvent::Room(BEvent::Cove(name, event)) => {
                        self.handle_cove_event(name, event).await?
                    }
                    UiEvent::Redraw => EventHandleResult::Continue,
                };
                match result {
                    EventHandleResult::Continue => {}
                    EventHandleResult::Stop => return Ok(()),
                }
                event = match event_rx.try_recv() {
                    Ok(event) => event,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                };
            }
        }
    }

    async fn handle_key_event(&mut self, event: KeyEvent) -> anyhow::Result<EventHandleResult> {
        const CONTINUE: anyhow::Result<EventHandleResult> = Ok(EventHandleResult::Continue);
        const STOP: anyhow::Result<EventHandleResult> = Ok(EventHandleResult::Stop);

        // Overlay
        if let Some(overlay) = &mut self.overlay {
            let reaction = match overlay {
                Overlay::SwitchRoom(state) => state.handle_key(event),
            };
            if let Some(reaction) = reaction {
                self.handle_overlay_reaction(reaction).await;
            }
            return CONTINUE;
        }

        // Main panel
        // TODO Implement

        // Otherwise, global bindings
        match event.code {
            KeyCode::Char('Q') => STOP,
            KeyCode::Char('s') => {
                self.overlay = Some(Overlay::SwitchRoom(SwitchRoomState::default()));
                CONTINUE
            }
            KeyCode::Char('J') => {
                self.switch_to_next_room();
                CONTINUE
            }
            KeyCode::Char('K') => {
                self.switch_to_prev_room();
                CONTINUE
            }
            KeyCode::Char('D') => {
                self.remove_current_room();
                CONTINUE
            }
            _ => CONTINUE,
        }
    }

    async fn handle_overlay_reaction(&mut self, reaction: OverlayReaction) {
        match reaction {
            OverlayReaction::Handled => {}
            OverlayReaction::Close => self.overlay = None,
            OverlayReaction::SwitchRoom(id) => {
                self.overlay = None;
                self.switch_to_room(id).await;
            }
        }
    }

    async fn handle_mouse_event(&mut self, event: MouseEvent) -> anyhow::Result<EventHandleResult> {
        let rooms_width = event.column;
        let users_width = self.last_area.width - event.column - 1;
        let rooms_hover = rooms_width == self.rooms_pane.width();
        let users_hover = users_width == self.users_pane.width();
        match event.kind {
            MouseEventKind::Moved => {
                self.rooms_pane.hover(rooms_hover);
                self.users_pane.hover(users_hover);
            }
            MouseEventKind::Down(_) => {
                self.rooms_pane.drag(rooms_hover);
                self.users_pane.drag(users_hover);
            }
            MouseEventKind::Up(_) => {
                self.rooms_pane.drag(false);
                self.users_pane.drag(false);
            }
            MouseEventKind::Drag(_) => {
                self.rooms_pane.drag_to(rooms_width);
                self.users_pane.drag_to(users_width);
            }
            // MouseEventKind::ScrollDown => todo!(),
            // MouseEventKind::ScrollUp => todo!(),
            _ => {}
        }
        Ok(EventHandleResult::Continue)
    }

    async fn handle_cove_event(
        &mut self,
        name: String,
        event: CoveEvent,
    ) -> anyhow::Result<EventHandleResult> {
        match event {
            CoveEvent::StateChanged => {}
            CoveEvent::IdentificationRequired => {
                // TODO Send identification if default nick is set in config
            }
        }
        Ok(EventHandleResult::Continue)
    }

    async fn render(&mut self, frame: &mut Frame<'_, Backend>) -> anyhow::Result<()> {
        let entire_area = frame.size();
        let areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(self.rooms_pane.width()),
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(self.users_pane.width()),
            ])
            .split(entire_area);
        let rooms_pane_area = areas[0];
        let rooms_pane_border = areas[1];
        let main_pane_area = areas[2];
        let users_pane_border = areas[3];
        let users_pane_area = areas[4];

        // Main pane and users pane
        self.render_room(frame, main_pane_area, users_pane_area)
            .await;

        // Rooms pane
        let mut rooms = Rooms::new(&self.cove_rooms);
        if let Some(RoomId::Cove(name)) = &self.room {
            rooms = rooms.select(name);
        }
        frame.render_widget(rooms, rooms_pane_area);

        // Pane borders and width
        self.rooms_pane.restrict_width(rooms_pane_area.width);
        frame.render_widget(self.rooms_pane.border(), rooms_pane_border);
        self.users_pane.restrict_width(users_pane_area.width);
        frame.render_widget(self.users_pane.border(), users_pane_border);

        // Overlay
        if let Some(overlay) = &mut self.overlay {
            match overlay {
                Overlay::SwitchRoom(state) => {
                    frame.render_stateful_widget(SwitchRoom, entire_area, state);
                    let (x, y) = state.last_cursor_pos();
                    frame.set_cursor(x, y);
                }
            }
        }

        Ok(())
    }

    async fn render_room(
        &mut self,
        frame: &mut Frame<'_, Backend>,
        main_pane_area: Rect,
        users_pane_area: Rect,
    ) {
        match &self.room {
            Some(RoomId::Cove(name)) => {
                if let Some(ui) = self.cove_rooms.get_mut(name) {
                    ui.render_main(frame, main_pane_area).await;
                    ui.render_users(frame, users_pane_area).await;
                } else {
                    self.room = None;
                }
            }
            None => {
                // TODO Render welcome screen
            }
        }
    }

    async fn switch_to_room(&mut self, id: RoomId) {
        match &id {
            RoomId::Cove(name) => {
                if let Entry::Vacant(entry) = self.cove_rooms.entry(name.clone()) {
                    let room =
                        CoveRoom::new(self.config, self.event_tx.clone(), name.clone()).await;
                    entry.insert(CoveUi::new(room));
                }
            }
        }
        self.room = Some(id);
    }

    fn rooms_in_order(&self) -> Vec<RoomId> {
        let mut rooms = vec![];
        rooms.extend(self.cove_rooms.keys().cloned().map(RoomId::Cove));
        rooms.sort();
        rooms
    }

    fn get_room_index(&self, rooms: &[RoomId]) -> Option<(usize, RoomId)> {
        let id = self.room.clone()?;
        let index = rooms.iter().position(|room| room == &id)?;
        Some((index, id))
    }

    fn set_room_index(&mut self, rooms: &[RoomId], index: usize) {
        if rooms.is_empty() {
            self.room = None;
            return;
        }

        let id = rooms[index % rooms.len()].clone();
        self.room = Some(id);
    }

    fn switch_to_next_room(&mut self) {
        let rooms = self.rooms_in_order();
        if let Some((index, _)) = self.get_room_index(&rooms) {
            self.set_room_index(&rooms, index + 1);
        }
    }

    fn switch_to_prev_room(&mut self) {
        let rooms = self.rooms_in_order();
        if let Some((index, _)) = self.get_room_index(&rooms) {
            self.set_room_index(&rooms, index + rooms.len() - 1);
        }
    }

    fn remove_current_room(&mut self) {
        let rooms = self.rooms_in_order();
        if let Some((index, id)) = self.get_room_index(&rooms) {
            match id {
                RoomId::Cove(name) => self.cove_rooms.remove(&name),
            };

            let rooms = self.rooms_in_order();
            let max_index = if rooms.is_empty() { 0 } else { rooms.len() - 1 };
            self.set_room_index(&rooms, index.min(max_index));
        }
    }
}
