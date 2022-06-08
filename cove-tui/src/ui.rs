use std::collections::hash_map::Entry;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, MouseEvent};
use crossterm::style::ContentStyle;
use futures::StreamExt;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use toss::frame::{Frame, Pos};
use toss::terminal::{Redraw, Terminal};

#[derive(Debug)]
pub enum UiEvent {
    Redraw,
    Term(Event),
}

enum EventHandleResult {
    Continue,
    Stop,
}

pub struct Ui {
    event_tx: UnboundedSender<UiEvent>,
}

impl Ui {
    fn new(event_tx: UnboundedSender<UiEvent>) -> Self {
        Self { event_tx }
    }

    pub async fn run(terminal: &mut Terminal) -> anyhow::Result<()> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let mut ui = Self::new(event_tx.clone());

        let result = tokio::select! {
            e = ui.run_main(terminal, event_tx.clone(), event_rx) => e,
            e = Self::shovel_crossterm_events(event_tx) => e,
        };
        result
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
        terminal: &mut Terminal,
        event_tx: UnboundedSender<UiEvent>,
        mut event_rx: UnboundedReceiver<UiEvent>,
    ) -> anyhow::Result<()> {
        loop {
            // 1. Render current state
            terminal.autoresize()?;
            self.render(terminal.frame()).await?;
            if terminal.present()? == Redraw::Required {
                event_tx.send(UiEvent::Redraw);
            }

            // 2. Handle events (in batches)
            let mut event = match event_rx.recv().await {
                Some(event) => event,
                None => return Ok(()),
            };
            loop {
                let result = match event {
                    UiEvent::Redraw => EventHandleResult::Continue,
                    UiEvent::Term(Event::Key(event)) => self.handle_key_event(event).await,
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
        frame.write(Pos::new(0, 0), "Hello world!", ContentStyle::default());

        Ok(())
    }

    async fn handle_key_event(&mut self, event: KeyEvent) -> EventHandleResult {
        match event.code {
            KeyCode::Char('Q') => return EventHandleResult::Stop,
            _ => {}
        }

        EventHandleResult::Continue
    }

    async fn handle_mouse_event(&mut self, event: MouseEvent) -> anyhow::Result<EventHandleResult> {
        Ok(EventHandleResult::Continue)
    }
}
