use std::collections::HashMap;
use std::sync::Arc;

use cove_core::conn::ConnTx;
use cove_core::{Session, SessionId};
use tokio::sync::oneshot::{self, Sender};
use tokio::sync::Mutex;

pub enum ConnectedState {
    ChoosingNick,
    Identifying,
    Online,
}

pub enum RoomState {
    Connecting,
    Reconnecting,
    Connected { state: ConnectedState, tx: ConnTx },
    DoesNotExist,
}

pub struct Room {
    name: String,
    state: RoomState,
    nick: Option<String>,
    others: HashMap<SessionId, Session>,
    stop: Sender<()>,
}

impl Room {
    pub async fn create(name: String) -> Arc<Mutex<Self>> {
        let (tx, rx) = oneshot::channel();

        let room = Self {
            name,
            state: RoomState::Connecting,
            nick: None,
            others: HashMap::new(),
            stop: tx,
        };
        let room = Arc::new(Mutex::new(room));

        let room_clone = room.clone();
        tokio::spawn(async {
            tokio::select! {
                _ = rx => {},
                _ = Self::connect(room_clone) => {}
            }
        });

        room
    }

    async fn connect(room: Arc<Mutex<Self>>) {
        todo!()
    }

    pub fn stop(self) {
        // If the send goes wrong because the other end has hung up, it's
        // already stopped and there's nothing to do.
        let _ = self.stop.send(());
    }
}
