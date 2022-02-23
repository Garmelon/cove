use std::io::Stdout;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, MouseEvent};
use futures::StreamExt;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui::backend::CrosstermBackend;
use tui::widgets::Paragraph;
use tui::{Frame, Terminal};

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

pub struct Ui {
    event_tx: UnboundedSender<UiEvent>,
    rooms_width: i32,
    log: Vec<String>,
}

impl Ui {
    fn new(event_tx: UnboundedSender<UiEvent>) -> Self {
        Self {
            event_tx,
            rooms_width: 24,
            log: vec!["Hello world!".to_string()],
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

            // 2. Handle events
            let event = event_rx.recv().await;
            self.log.push(format!("{event:?}"));
            let result = match event {
                Some(UiEvent::Term(Event::Key(event))) => self.handle_key_event(event).await?,
                Some(UiEvent::Term(Event::Mouse(event))) => self.handle_mouse_event(event).await?,
                Some(UiEvent::Term(Event::Resize(_, _))) => EventHandleResult::Continue,
                Some(UiEvent::Redraw) => EventHandleResult::Continue,
                None => EventHandleResult::Stop,
            };
            match result {
                EventHandleResult::Continue => {}
                EventHandleResult::Stop => break Ok(()),
            }
        }
    }

    async fn handle_key_event(&mut self, event: KeyEvent) -> anyhow::Result<EventHandleResult> {
        Ok(match event.code {
            // KeyCode::Backspace => todo!(),
            // KeyCode::Enter => todo!(),
            // KeyCode::Left => todo!(),
            // KeyCode::Right => todo!(),
            // KeyCode::Up => todo!(),
            // KeyCode::Down => todo!(),
            // KeyCode::Home => todo!(),
            // KeyCode::End => todo!(),
            // KeyCode::PageUp => todo!(),
            // KeyCode::PageDown => todo!(),
            // KeyCode::Tab => todo!(),
            // KeyCode::BackTab => todo!(),
            // KeyCode::Delete => todo!(),
            // KeyCode::Insert => todo!(),
            // KeyCode::F(_) => todo!(),
            // KeyCode::Char(_) => todo!(),
            // KeyCode::Null => todo!(),
            KeyCode::Esc => EventHandleResult::Stop,
            _ => EventHandleResult::Continue,
        })
    }

    async fn handle_mouse_event(&mut self, event: MouseEvent) -> anyhow::Result<EventHandleResult> {
        Ok(match event.kind {
            // MouseEventKind::Down(_) => todo!(),
            // MouseEventKind::Up(_) => todo!(),
            // MouseEventKind::Drag(_) => todo!(),
            // MouseEventKind::Moved => todo!(),
            // MouseEventKind::ScrollDown => todo!(),
            // MouseEventKind::ScrollUp => todo!(),
            _ => EventHandleResult::Continue,
        })
    }

    async fn render(&mut self, frame: &mut Frame<'_, Backend>) -> anyhow::Result<()> {
        let scroll = if self.log.len() as u16 > frame.size().height {
            self.log.len() as u16 - frame.size().height
        } else {
            0
        };
        frame.render_widget(
            Paragraph::new(self.log.join("\n")).scroll((scroll, 0)),
            frame.size(),
        );
        Ok(())
    }
}
