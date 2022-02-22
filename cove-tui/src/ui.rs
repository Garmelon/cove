use std::collections::HashMap;
use std::io::Stdout;
use std::sync::Arc;

use tokio::sync::Mutex;
use tui::backend::CrosstermBackend;
use tui::widgets::Paragraph;
use tui::Terminal;

use crate::room::Room;

pub enum Overlay {
    Error(String),
    ChooseRoom(String),
}

#[derive(Default)]
pub struct Ui {
    rooms: HashMap<String, Arc<Mutex<Room>>>,
    room: Option<Arc<Mutex<Room>>>,
    overlay: Option<Overlay>,
}

impl Ui {
    pub async fn render_to_terminal(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> anyhow::Result<()> {
        terminal.autoresize()?;

        let mut frame = terminal.get_frame();
        frame.render_widget(Paragraph::new("Hello world!"), frame.size());

        // Do a little dance to please the borrow checker
        let cursor = frame.cursor();
        drop(frame);
        terminal.set_cursor_opt(cursor)?;

        terminal.flush()?;
        terminal.flush_backend()?;
        terminal.swap_buffers();

        Ok(())
    }
}
