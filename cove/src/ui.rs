mod chat;
mod euph;
mod key_bindings;
mod rooms;
mod util;
mod widgets;

use std::convert::Infallible;
use std::io;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use cove_config::Config;
use cove_input::InputEvent;
use parking_lot::FairMutex;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task;
use toss::widgets::BoxedAsync;
use toss::{Terminal, WidgetExt};

use crate::logger::{LogMsg, Logger};
use crate::macros::logging_unwrap;
use crate::util::InfallibleExt;
use crate::vault::Vault;

pub use self::chat::ChatMsg;
use self::chat::ChatState;
use self::rooms::Rooms;
use self::widgets::ListState;

/// Time to spend batch processing events before redrawing the screen.
const EVENT_PROCESSING_TIME: Duration = Duration::from_millis(1000 / 15); // 15 fps

/// Error for anything that can go wrong while rendering.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    #[error("{0}")]
    Vault(#[from] vault::tokio::Error<rusqlite::Error>),
    #[error("{0}")]
    Io(#[from] io::Error),
}

impl From<Infallible> for UiError {
    fn from(value: Infallible) -> Self {
        Err(value).infallible()
    }
}

pub enum UiEvent {
    GraphemeWidthsChanged,
    LogChanged,
    Term(crossterm::event::Event),
    Euph(euphoxide::bot::instance::Event),
}

enum EventHandleResult {
    Redraw,
    Continue,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Main,
    Log,
}

pub struct Ui {
    config: &'static Config,
    event_tx: UnboundedSender<UiEvent>,

    mode: Mode,

    rooms: Rooms,
    log_chat: ChatState<LogMsg, Logger>,

    key_bindings_visible: bool,
    key_bindings_list: ListState<Infallible>,
}

impl Ui {
    const POLL_DURATION: Duration = Duration::from_millis(100);

    pub async fn run(
        config: &'static Config,
        terminal: &mut Terminal,
        vault: Vault,
        logger: Logger,
        logger_rx: UnboundedReceiver<()>,
    ) -> anyhow::Result<()> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let crossterm_lock = Arc::new(FairMutex::new(()));

        // Prepare and start crossterm event polling task
        let weak_crossterm_lock = Arc::downgrade(&crossterm_lock);
        let event_tx_clone = event_tx.clone();
        let crossterm_event_task = task::spawn_blocking(|| {
            Self::poll_crossterm_events(event_tx_clone, weak_crossterm_lock)
        });

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
            config,
            event_tx: event_tx.clone(),
            mode: Mode::Main,
            rooms: Rooms::new(config, vault, event_tx.clone()).await,
            log_chat: ChatState::new(logger),
            key_bindings_visible: false,
            key_bindings_list: ListState::new(),
        };
        tokio::select! {
            e = ui.run_main(terminal, event_rx, crossterm_lock) => e?,
            _ = Self::update_on_log_event(logger_rx, &event_tx) => (),
            e = crossterm_event_task => e??,
        }
        Ok(())
    }

    fn poll_crossterm_events(
        tx: UnboundedSender<UiEvent>,
        lock: Weak<FairMutex<()>>,
    ) -> io::Result<()> {
        loop {
            let Some(lock) = lock.upgrade() else {
                return Ok(());
            };
            let _guard = lock.lock();
            if crossterm::event::poll(Self::POLL_DURATION)? {
                let event = crossterm::event::read()?;
                if tx.send(UiEvent::Term(event)).is_err() {
                    return Ok(());
                }
            }
        }
    }

    async fn update_on_log_event(
        mut logger_rx: UnboundedReceiver<()>,
        event_tx: &UnboundedSender<UiEvent>,
    ) {
        loop {
            if logger_rx.recv().await.is_none() {
                return;
            }
            if event_tx.send(UiEvent::LogChanged).is_err() {
                return;
            }
        }
    }

    async fn run_main(
        &mut self,
        terminal: &mut Terminal,
        mut event_rx: UnboundedReceiver<UiEvent>,
        crossterm_lock: Arc<FairMutex<()>>,
    ) -> Result<(), UiError> {
        let mut redraw = true;

        loop {
            // Redraw if necessary
            if redraw {
                redraw = false;
                terminal.present_async_widget(self.widget().await).await?;

                if terminal.measuring_required() {
                    let _guard = crossterm_lock.lock();
                    terminal.measure_widths()?;
                    if self.event_tx.send(UiEvent::GraphemeWidthsChanged).is_err() {
                        return Ok(());
                    }
                }
            }

            // Handle events (in batches)
            let mut event = match event_rx.recv().await {
                Some(event) => event,
                None => return Ok(()),
            };
            let end_time = Instant::now() + EVENT_PROCESSING_TIME;
            loop {
                match self.handle_event(terminal, &crossterm_lock, event).await {
                    EventHandleResult::Redraw => redraw = true,
                    EventHandleResult::Continue => {}
                    EventHandleResult::Stop => return Ok(()),
                }
                if Instant::now() >= end_time {
                    break;
                }
                event = match event_rx.try_recv() {
                    Ok(event) => event,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                };
            }
        }
    }

    async fn widget(&mut self) -> BoxedAsync<'_, UiError> {
        let widget = match self.mode {
            Mode::Main => self.rooms.widget().await,
            Mode::Log => self.log_chat.widget(String::new(), true),
        };

        if self.key_bindings_visible {
            let popup = key_bindings::widget(&mut self.key_bindings_list, self.config);
            popup.desync().above(widget).boxed_async()
        } else {
            widget
        }
    }

    async fn handle_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: UiEvent,
    ) -> EventHandleResult {
        match event {
            UiEvent::GraphemeWidthsChanged => EventHandleResult::Redraw,
            UiEvent::LogChanged if self.mode == Mode::Log => EventHandleResult::Redraw,
            UiEvent::LogChanged => EventHandleResult::Continue,
            UiEvent::Term(crossterm::event::Event::Resize(_, _)) => EventHandleResult::Redraw,
            UiEvent::Term(event) => {
                self.handle_term_event(terminal, crossterm_lock.clone(), event)
                    .await
            }
            UiEvent::Euph(event) => {
                if self.rooms.handle_euph_event(event).await {
                    EventHandleResult::Redraw
                } else {
                    EventHandleResult::Continue
                }
            }
        }
    }

    async fn handle_term_event(
        &mut self,
        terminal: &mut Terminal,
        crossterm_lock: Arc<FairMutex<()>>,
        event: crossterm::event::Event,
    ) -> EventHandleResult {
        let mut event = InputEvent::new(event, terminal, crossterm_lock);
        let keys = &self.config.keys;

        if event.matches(&keys.general.exit) {
            return EventHandleResult::Stop;
        }

        // Key bindings list overrides any other bindings if visible
        if self.key_bindings_visible {
            if event.matches(&keys.general.abort) || event.matches(&keys.general.help) {
                self.key_bindings_visible = false;
                return EventHandleResult::Redraw;
            }
            if key_bindings::handle_input_event(&mut self.key_bindings_list, &mut event, keys) {
                return EventHandleResult::Redraw;
            }
            // ... and does not let anything below the popup receive events
            return EventHandleResult::Continue;
        }

        if event.matches(&keys.general.help) {
            self.key_bindings_visible = true;
            return EventHandleResult::Redraw;
        }

        match self.mode {
            Mode::Main => {
                if event.matches(&keys.general.log) {
                    self.mode = Mode::Log;
                    return EventHandleResult::Redraw;
                }

                if self.rooms.handle_input_event(&mut event, keys).await {
                    return EventHandleResult::Redraw;
                }
            }
            Mode::Log => {
                if event.matches(&keys.general.abort) || event.matches(&keys.general.log) {
                    self.mode = Mode::Main;
                    return EventHandleResult::Redraw;
                }

                let reaction = self
                    .log_chat
                    .handle_input_event(&mut event, keys, false)
                    .await;
                let reaction = logging_unwrap!(reaction);
                if reaction.handled() {
                    return EventHandleResult::Redraw;
                }
            }
        }

        EventHandleResult::Continue
    }
}
