mod input;
mod layout;
mod overlays;
mod rooms;
mod textline;

use std::collections::HashMap;
use std::io::Stdout;
use std::sync::Arc;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use futures::StreamExt;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::{Frame, Terminal};

use crate::room::Room;
use crate::ui::overlays::OverlayReaction;

use self::input::EventHandler;
use self::overlays::{JoinRoom, JoinRoomState};
use self::rooms::{Rooms, RoomsState};

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
    event_tx: UnboundedSender<UiEvent>,
    rooms: HashMap<String, Arc<Mutex<Room>>>,
    rooms_state: RoomsState,
    overlay: Option<Overlay>,
}

impl Ui {
    fn new(event_tx: UnboundedSender<UiEvent>) -> Self {
        Self {
            event_tx,
            rooms: HashMap::new(),
            rooms_state: RoomsState::default(),
            overlay: None,
        }
    }

    pub async fn run(terminal: &mut Terminal<Backend>) -> anyhow::Result<()> {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mut ui = Self::new(event_tx.clone());

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
            self.render(&mut frame).await?;

            // Do a little dance to please the borrow checker
            let cursor = frame.cursor();
            drop(frame);
            terminal.set_cursor_opt(cursor)?;

            terminal.flush()?;
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
                match reaction {
                    OverlayReaction::Handled => {}
                    OverlayReaction::Close => self.overlay = None,
                    OverlayReaction::JoinRoom(name) => todo!(),
                }
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

    async fn handle_mouse_event(&mut self, event: MouseEvent) -> anyhow::Result<EventHandleResult> {
        let rooms_width = event.column + 1;
        let over_rooms = self.rooms_state.width() == rooms_width;
        match event.kind {
            MouseEventKind::Moved => self.rooms_state.hover(over_rooms),
            MouseEventKind::Down(_) => self.rooms_state.drag(over_rooms),
            MouseEventKind::Up(_) => self.rooms_state.drag(false),
            MouseEventKind::Drag(_) => self.rooms_state.drag_to(rooms_width),
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
                Constraint::Length(self.rooms_state.width()), // Rooms list
                Constraint::Min(1),                           // Main panel
            ])
            .split(entire_area);
        let rooms_list_area = areas[0];
        let main_panel_area = areas[1];

        // Rooms list
        frame.render_stateful_widget(
            Rooms::new(&self.rooms),
            rooms_list_area,
            &mut self.rooms_state,
        );

        // Main panel
        // TODO Implement

        // Overlays
        if let Some(overlay) = &mut self.overlay {
            match overlay {
                Overlay::JoinRoom(state) => {
                    frame.render_stateful_widget(JoinRoom, entire_area, state)
                }
            }
        }

        Ok(())
    }
}
