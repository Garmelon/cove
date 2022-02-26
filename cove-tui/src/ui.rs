mod input;
mod layout;
mod overlays;
mod pane;
mod room;
mod rooms;
mod textline;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io::Stdout;
use std::sync::Arc;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use futures::StreamExt;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::{Frame, Terminal};

use crate::config::Config;
use crate::room::Room;
use crate::ui::overlays::OverlayReaction;

use self::input::EventHandler;
use self::overlays::{JoinRoom, JoinRoomState};
use self::pane::PaneInfo;
use self::room::RoomInfo;
use self::rooms::Rooms;

pub type Backend = CrosstermBackend<Stdout>;

#[derive(Debug)]
pub enum UiEvent {
    Term(Event),
    Redraw,
}

enum EventHandleResult {
    Continue,
    Stop,
}

enum Overlay {
    JoinRoom(JoinRoomState),
}

pub struct Ui {
    config: &'static Config,
    event_tx: UnboundedSender<UiEvent>,
    rooms: HashMap<String, Arc<Mutex<Room>>>,

    rooms_pane: PaneInfo,
    users_pane: PaneInfo,

    room: Option<RoomInfo>,
    overlay: Option<Overlay>,

    last_area: Rect,
}

impl Ui {
    fn new(config: &'static Config, event_tx: UnboundedSender<UiEvent>) -> Self {
        Self {
            config,
            event_tx,
            rooms: HashMap::new(),

            rooms_pane: PaneInfo::default(),
            users_pane: PaneInfo::default(),

            room: None,
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
                    UiEvent::Term(Event::Key(event)) => self.handle_key_event(event).await?,
                    UiEvent::Term(Event::Mouse(event)) => self.handle_mouse_event(event).await?,
                    UiEvent::Term(Event::Resize(_, _)) => EventHandleResult::Continue,
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
                Overlay::JoinRoom(state) => state.handle_key(event),
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
            KeyCode::Char('q') => STOP,
            KeyCode::Char('c') => {
                self.overlay = Some(Overlay::JoinRoom(JoinRoomState::default()));
                CONTINUE
            }
            _ => CONTINUE,
        }
    }

    async fn handle_overlay_reaction(&mut self, reaction: OverlayReaction) {
        match reaction {
            OverlayReaction::Handled => {}
            OverlayReaction::Close => self.overlay = None,
            OverlayReaction::JoinRoom(name) => {
                let name = name.trim();
                if !name.is_empty() {
                    self.overlay = None;
                    self.switch_to_room(name.to_string()).await;
                }
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

        // Rooms pane
        frame.render_widget(Rooms::new(&self.rooms), rooms_pane_area);

        // TODO Main pane and users pane

        // Pane borders and width
        self.rooms_pane.restrict_width(rooms_pane_area.width);
        frame.render_widget(self.rooms_pane.border(), rooms_pane_border);
        self.users_pane.restrict_width(users_pane_area.width);
        frame.render_widget(self.users_pane.border(), users_pane_border);

        // Overlays
        if let Some(overlay) = &mut self.overlay {
            match overlay {
                Overlay::JoinRoom(state) => {
                    frame.render_stateful_widget(JoinRoom, entire_area, state);
                    let (x, y) = state.last_cursor_pos();
                    frame.set_cursor(x, y);
                }
            }
        }

        Ok(())
    }

    async fn switch_to_room(&mut self, name: String) {
        let room = match self.rooms.entry(name.clone()) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let identity = self.config.cove_identity.clone();
                let room = Room::new(name.clone(), identity, None, self.config).await;
                entry.insert(room.clone());
                room
            }
        };

        self.room = Some(RoomInfo::new(name, room))
    }
}
