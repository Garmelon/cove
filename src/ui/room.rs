use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use crossterm::style::ContentStyle;
use parking_lot::FairMutex;
use tokio::sync::mpsc;
use toss::frame::{Frame, Pos, Size};
use toss::terminal::Terminal;

use crate::chat::Chat;
use crate::euph::{self, Status};
use crate::vault::{EuphMsg, EuphVault};

use super::{util, UiEvent};

pub struct EuphRoom {
    ui_event_tx: mpsc::UnboundedSender<UiEvent>,
    room: Option<euph::Room>,
    chat: Chat<EuphMsg, EuphVault>,
}

impl EuphRoom {
    pub fn new(vault: EuphVault, ui_event_tx: mpsc::UnboundedSender<UiEvent>) -> Self {
        Self {
            ui_event_tx,
            room: None,
            chat: Chat::new(vault),
        }
    }

    pub fn connect(&mut self) {
        if self.room.is_none() {
            self.room = Some(euph::Room::new(
                self.chat.store().clone(),
                self.ui_event_tx.clone(),
            ));
        }
    }

    pub fn disconnect(&mut self) {
        self.room = None;
    }

    pub fn connected(&self) -> bool {
        if let Some(room) = &self.room {
            !room.stopped()
        } else {
            false
        }
    }

    pub fn retain(&mut self) {
        if let Some(room) = &self.room {
            if room.stopped() {
                self.room = None;
            }
        }
    }

    pub async fn render(&mut self, frame: &mut Frame) {
        let size = frame.size();

        let chat_pos = Pos::new(0, 2);
        let chat_size = Size {
            height: size.height - 2,
            ..size
        };
        self.chat.render(frame, chat_pos, chat_size).await;

        let room = self.chat.store().room();
        let status = if let Some(room) = &self.room {
            room.status().await.ok()
        } else {
            None
        };
        Self::render_top_bar(frame, room, status);
    }

    fn render_top_bar(frame: &mut Frame, room: &str, status: Option<Option<Status>>) {
        // Clear area in case something accidentally wrote on it already
        let size = frame.size();
        for x in 0..size.width as i32 {
            frame.write(Pos::new(x, 0), " ", ContentStyle::default());
            frame.write(Pos::new(x, 1), "─", ContentStyle::default());
        }

        // Write status
        let status = match status {
            None => format!("&{room}, archive"),
            Some(None) => format!("&{room}, connecting..."),
            Some(Some(Status::Joining(j))) => {
                if j.bounce.is_none() {
                    format!("&{room}, joining...")
                } else {
                    format!("&{room}, auth required")
                }
            }
            Some(Some(Status::Joined(j))) => {
                let nick = &j.session.name;
                if nick.is_empty() {
                    format!("&{room}, present without nick")
                } else {
                    format!("&{room}, present as {nick}",)
                }
            }
        };
        frame.write(Pos::new(0, 0), &status, ContentStyle::default());
    }

    pub async fn handle_key_event(
        &mut self,
        terminal: &mut Terminal,
        size: Size,
        crossterm_lock: &Arc<FairMutex<()>>,
        event: KeyEvent,
    ) {
        let chat_size = Size {
            height: size.height - 2,
            ..size
        };
        self.chat
            .handle_navigation(terminal, chat_size, event)
            .await;

        if let Some(room) = &self.room {
            if let Ok(Some(Status::Joined(_))) = room.status().await {
                if let KeyCode::Char('n' | 'N') = event.code {
                    if let Some(new_nick) = util::prompt(terminal, crossterm_lock) {
                        let _ = room.nick(new_nick);
                    }
                }

                if let Some((parent, content)) = self
                    .chat
                    .handle_messaging(terminal, crossterm_lock, event)
                    .await
                {
                    let _ = room.send(parent, content);
                }
            }
        }
    }
}
