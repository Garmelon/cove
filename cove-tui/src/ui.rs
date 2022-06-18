use std::sync::{Arc, Weak};
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use parking_lot::FairMutex;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task;
use toss::frame::{Frame, Pos, Size};
use toss::terminal::Terminal;

use crate::chat::Chat;
use crate::log::{Log, LogMsg};
use crate::store::dummy::{DummyMsg, DummyStore};

#[derive(Debug)]
pub enum UiEvent {
    Redraw,
    Term(Event),
}

enum EventHandleResult {
    Continue,
    Stop,
}

enum Visible {
    Main,
    Log,
}

pub struct Ui {
    event_tx: UnboundedSender<UiEvent>,
    log: Log,

    visible: Visible,
    chat: Chat<DummyMsg, DummyStore>,
    log_chat: Chat<LogMsg, Log>,
}

impl Ui {
    const POLL_DURATION: Duration = Duration::from_millis(100);

    pub async fn run(terminal: &mut Terminal) -> anyhow::Result<()> {
        let log = Log::new();
        log.log("Hello", "world!");

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let crossterm_lock = Arc::new(FairMutex::new(()));

        // Prepare and start crossterm event polling task
        let weak_crossterm_lock = Arc::downgrade(&crossterm_lock);
        let event_tx_clone = event_tx.clone();
        let crossterm_event_task = task::spawn_blocking(|| {
            Self::poll_crossterm_events(event_tx_clone, weak_crossterm_lock)
        });
        log.log("main", "Started input polling task");

        // Prepare dummy message store and chat for testing
        let store = DummyStore::new()
            .msg(DummyMsg::new(1, "nick", "content"))
            .msg(DummyMsg::new(2, "Some1Else", "reply").parent(1))
            .msg(DummyMsg::new(3, "Some1Else", "deeper reply").parent(2))
            .msg(DummyMsg::new(4, "abc123", "even deeper reply").parent(3))
            .msg(DummyMsg::new(5, "Some1Else", "another reply").parent(1))
            .msg(DummyMsg::new(6, "Some1Else", "third reply").parent(1))
            .msg(DummyMsg::new(8, "nick", "reply to nothing").parent(7))
            .msg(DummyMsg::new(9, "nick", "another reply to nothing").parent(7))
            .msg(DummyMsg::new(10, "abc123", "reply to reply to nothing").parent(8))
            .msg(DummyMsg::new(11, "nick", "yet another reply to nothing").parent(7))
            .msg(DummyMsg::new(12, "abc123", "beep\nboop").parent(11));
        let chat = Chat::new(store);

        // Run main UI.
        //
        // If the run_main method exits at any point or if this `run` method is
        // not awaited any more, the crossterm_lock Arc should be deallocated,
        // meaning the crossterm_event_task will also stop after at most
        // `Self::POLL_DURATION`.
        //
        // On the other hand, if the crossterm_event_task stops for any reason,
        // the rest of the UI is also shut down and the client stops.
        let mut ui = Self {
            event_tx,
            log: log.clone(),
            visible: Visible::Log,
            chat,
            log_chat: Chat::new(log),
        };
        let result = tokio::select! {
            e = ui.run_main(terminal, event_rx, crossterm_lock) => e,
            Ok(e) = crossterm_event_task => e,
        };
        result
    }

    fn poll_crossterm_events(
        tx: UnboundedSender<UiEvent>,
        lock: Weak<FairMutex<()>>,
    ) -> anyhow::Result<()> {
        while let Some(lock) = lock.upgrade() {
            let _guard = lock.lock();
            if crossterm::event::poll(Self::POLL_DURATION)? {
                let event = crossterm::event::read()?;
                tx.send(UiEvent::Term(event))?;
            }
        }
        Ok(())
    }

    async fn run_main(
        &mut self,
        terminal: &mut Terminal,
        mut event_rx: UnboundedReceiver<UiEvent>,
        crossterm_lock: Arc<FairMutex<()>>,
    ) -> anyhow::Result<()> {
        loop {
            // 1. Render current state
            terminal.autoresize()?;
            self.render(terminal.frame()).await?;
            terminal.present()?;

            // 2. Measure widths if required
            if terminal.measuring_required() {
                let _guard = crossterm_lock.lock();
                terminal.measure_widths()?;
                self.event_tx.send(UiEvent::Redraw)?;
            }

            // 3. Handle events (in batches)
            let mut event = match event_rx.recv().await {
                Some(event) => event,
                None => return Ok(()),
            };
            terminal.autoresize()?;
            loop {
                let size = terminal.frame().size();
                let result = match event {
                    UiEvent::Redraw => EventHandleResult::Continue,
                    UiEvent::Term(Event::Key(event)) => {
                        self.handle_key_event(event, terminal, size, &crossterm_lock)
                            .await
                    }
                    UiEvent::Term(Event::Mouse(event)) => self.handle_mouse_event(event).await?,
                    UiEvent::Term(Event::Resize(_, _)) => EventHandleResult::Continue,
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

    async fn render(&mut self, frame: &mut Frame) -> anyhow::Result<()> {
        match self.visible {
            Visible::Main => self.chat.render(frame, Pos::new(0, 0), frame.size()).await,
            Visible::Log => {
                self.log_chat
                    .render(frame, Pos::new(0, 0), frame.size())
                    .await
            }
        }
        Ok(())
    }

    async fn handle_key_event(
        &mut self,
        event: KeyEvent,
        terminal: &mut Terminal,
        size: Size,
        crossterm_lock: &Arc<FairMutex<()>>,
    ) -> EventHandleResult {
        // Always exit when shift+q or ctrl+c are pressed
        let shift_q = event.code == KeyCode::Char('Q');
        let ctrl_c = event.modifiers == KeyModifiers::CONTROL && event.code == KeyCode::Char('c');
        if shift_q || ctrl_c {
            return EventHandleResult::Stop;
        }

        match event.code {
            KeyCode::Char('e') => self.log.log("EE E", "E ee e!"),
            KeyCode::F(1) => self.visible = Visible::Main,
            KeyCode::F(2) => self.visible = Visible::Log,
            _ => {}
        }

        match self.visible {
            Visible::Main => {
                self.chat
                    .handle_key_event(event, terminal, size, crossterm_lock)
                    .await;
            }
            Visible::Log => {
                self.log_chat
                    .handle_key_event(event, terminal, size, crossterm_lock)
                    .await;
            }
        }

        EventHandleResult::Continue
    }

    async fn handle_mouse_event(
        &mut self,
        _event: MouseEvent,
    ) -> anyhow::Result<EventHandleResult> {
        Ok(EventHandleResult::Continue)
    }
}
