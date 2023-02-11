// TODO Stop if room does not exist (e. g. 404)

use std::convert::Infallible;
use std::time::Duration;

use euphoxide::api::packet::ParsedPacket;
use euphoxide::api::{
    Auth, AuthOption, Data, Log, Login, Logout, MessageId, Nick, Send, SendEvent, SendReply, Time,
    UserId,
};
use euphoxide::bot::instance::{Event, Instance, InstanceConfig, Snapshot};
use euphoxide::conn::{self, ConnTx};
use log::{debug, error, info, warn};
use tokio::select;
use tokio::sync::oneshot;

use crate::macros::ok_or_return;
use crate::vault::EuphRoomVault;

const LOG_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum State {
    Disconnected,
    Connecting,
    Connected(ConnTx, conn::State),
    Stopped,
}

impl State {
    pub fn conn_tx(&self) -> Option<&ConnTx> {
        if let Self::Connected(conn_tx, _) = self {
            Some(conn_tx)
        } else {
            None
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not connected to room")]
    NotConnected,
}

#[derive(Debug)]
pub struct Room {
    vault: EuphRoomVault,
    ephemeral: bool,

    instance: Instance,
    state: State,

    /// `None` before any `snapshot-event`, then either `Some(None)` or
    /// `Some(Some(id))`. Reset whenever connection is lost.
    last_msg_id: Option<Option<MessageId>>,

    /// `Some` while `Self::regularly_request_logs` is running. Set to `None` to
    /// drop the sender and stop the task.
    log_request_canary: Option<oneshot::Sender<Infallible>>,
}

impl Room {
    pub fn new<F>(vault: EuphRoomVault, instance_config: InstanceConfig, on_event: F) -> Self
    where
        F: Fn(Event) + std::marker::Send + Sync + 'static,
    {
        // &rl2dev's message history is broken and requesting old messages past
        // a certain point results in errors. Cove should not keep retrying log
        // requests when hitting that limit, so &rl2dev is always opened in
        // ephemeral mode.
        let ephemeral = vault.vault().vault().ephemeral() || vault.room() == "rl2dev";

        Self {
            vault,
            ephemeral,
            instance: instance_config.build(on_event),
            state: State::Disconnected,
            last_msg_id: None,
            log_request_canary: None,
        }
    }

    pub fn stopped(&self) -> bool {
        self.instance.stopped()
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    fn conn_tx(&self) -> Result<&ConnTx, Error> {
        self.state.conn_tx().ok_or(Error::NotConnected)
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Connecting(_) => {
                self.state = State::Connecting;

                // Juuust to make sure
                self.last_msg_id = None;
                self.log_request_canary = None;
            }
            Event::Connected(_, Snapshot { conn_tx, state }) => {
                if !self.ephemeral {
                    let (tx, rx) = oneshot::channel();
                    self.log_request_canary = Some(tx);
                    let vault_clone = self.vault.clone();
                    let conn_tx_clone = conn_tx.clone();
                    debug!("{}: spawning log request task", self.instance.config().name);
                    tokio::task::spawn(async move {
                        select! {
                            _ = rx => {},
                            _ = Self::regularly_request_logs(vault_clone, conn_tx_clone) => {},
                        }
                    });
                }

                self.state = State::Connected(conn_tx, state);

                let cookies = &*self.instance.config().server.cookies;
                let cookies = cookies.lock().unwrap().clone();
                self.vault.vault().set_cookies(cookies);
            }
            Event::Packet(_, packet, Snapshot { conn_tx, state }) => {
                self.state = State::Connected(conn_tx, state);
                self.on_packet(packet);
            }
            Event::Disconnected(_) => {
                self.state = State::Disconnected;
                self.last_msg_id = None;
                self.log_request_canary = None;
            }
            Event::Stopped(_) => {
                // TODO Remove room somewhere if this happens? If it doesn't already happen during stabilization
                self.state = State::Stopped;
            }
        }
    }

    async fn regularly_request_logs(vault: EuphRoomVault, conn_tx: ConnTx) {
        // TODO Make log downloading smarter

        // Possible log-related mechanics. Some of these could also run in some
        // sort of "repair logs" mode that can be started via some key binding.
        // For now, this is just a list of ideas.
        //
        // Download room history until there are no more gaps between now and
        // the first known message.
        //
        // Download room history until reaching the beginning of the room's
        // history.
        //
        // Check if the last known message still exists on the server. If it
        // doesn't, do a binary search to find the server's last message and
        // delete all older messages.
        //
        // Untruncate messages in the history, as well as new messages.
        //
        // Try to retrieve messages that are not in the room log by retrieving
        // them by id.
        //
        // Redownload messages that are already known to find any edits and
        // deletions that happened while the client was offline.
        //
        // Delete messages marked as deleted as well as all their children.

        loop {
            tokio::time::sleep(LOG_INTERVAL).await;
            Self::request_logs(&vault, &conn_tx).await;
        }
    }

    async fn request_logs(vault: &EuphRoomVault, conn_tx: &ConnTx) {
        let before = match vault.last_span().await {
            Some((None, _)) => return, // Already at top of room history
            Some((Some(before), _)) => Some(before),
            None => None,
        };

        debug!("{}: requesting logs", vault.room());

        // &rl2dev's message history is broken and requesting old messages past
        // a certain point results in errors. By reducing the amount of messages
        // in each log request, we can get closer to this point. Since &rl2dev
        // is fairly low in activity, this should be fine.
        let n = if vault.room() == "rl2dev" { 50 } else { 1000 };

        let _ = conn_tx.send(Log { n, before }).await;
        // The code handling incoming events and replies also handles
        // `LogReply`s, so we don't need to do anything special here.
    }

    fn own_user_id(&self) -> Option<UserId> {
        if let State::Connected(_, state) = &self.state {
            Some(match state {
                conn::State::Joining(joining) => joining.hello.as_ref()?.session.id.clone(),
                conn::State::Joined(joined) => joined.session.id.clone(),
            })
        } else {
            None
        }
    }

    fn on_packet(&mut self, packet: ParsedPacket) {
        let instance_name = &self.instance.config().name;
        let data = ok_or_return!(&packet.content);
        match data {
            Data::BounceEvent(_) => {}
            Data::DisconnectEvent(_) => {}
            Data::HelloEvent(_) => {}
            Data::JoinEvent(d) => {
                debug!("{instance_name}: {:?} joined", d.0.name);
            }
            Data::LoginEvent(_) => {}
            Data::LogoutEvent(_) => {}
            Data::NetworkEvent(d) => {
                warn!("{instance_name}: network event ({})", d.r#type);
            }
            Data::NickEvent(d) => {
                debug!("{instance_name}: {:?} renamed to {:?}", d.from, d.to);
            }
            Data::EditMessageEvent(_) => {
                info!("{instance_name}: a message was edited");
            }
            Data::PartEvent(d) => {
                debug!("{instance_name}: {:?} left", d.0.name);
            }
            Data::PingEvent(_) => {}
            Data::PmInitiateEvent(d) => {
                // TODO Show info popup and automatically join PM room
                info!(
                    "{instance_name}: {:?} initiated a pm from &{}",
                    d.from_nick, d.from_room
                );
            }
            Data::SendEvent(SendEvent(msg)) => {
                let own_user_id = self.own_user_id();
                if let Some(last_msg_id) = &mut self.last_msg_id {
                    self.vault
                        .add_msg(Box::new(msg.clone()), *last_msg_id, own_user_id);
                    *last_msg_id = Some(msg.id);
                }
            }
            Data::SnapshotEvent(d) => {
                info!("{instance_name}: successfully joined");
                self.vault.join(Time::now());
                self.last_msg_id = Some(d.log.last().map(|m| m.id));
                self.vault.add_msgs(d.log.clone(), None, self.own_user_id());
            }
            Data::LogReply(d) => {
                self.vault
                    .add_msgs(d.log.clone(), d.before, self.own_user_id());
            }
            Data::SendReply(SendReply(msg)) => {
                let own_user_id = self.own_user_id();
                if let Some(last_msg_id) = &mut self.last_msg_id {
                    self.vault
                        .add_msg(Box::new(msg.clone()), *last_msg_id, own_user_id);
                    *last_msg_id = Some(msg.id);
                }
            }
            _ => {}
        }
    }

    pub fn auth(&self, password: String) -> Result<(), Error> {
        self.conn_tx()?.send_only(Auth {
            r#type: AuthOption::Passcode,
            passcode: Some(password),
        });
        Ok(())
    }

    pub fn log(&self) -> Result<(), Error> {
        let conn_tx_clone = self.conn_tx()?.clone();
        let vault_clone = self.vault.clone();
        tokio::task::spawn(async move { Self::request_logs(&vault_clone, &conn_tx_clone).await });
        Ok(())
    }

    pub fn nick(&self, name: String) -> Result<(), Error> {
        self.conn_tx()?.send_only(Nick { name });
        Ok(())
    }

    pub fn send(
        &self,
        parent: Option<MessageId>,
        content: String,
    ) -> Result<oneshot::Receiver<MessageId>, Error> {
        let reply = self.conn_tx()?.send(Send { content, parent });
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            if let Ok(reply) = reply.await {
                let _ = tx.send(reply.0.id);
            }
        });
        Ok(rx)
    }

    pub fn login(&self, email: String, password: String) -> Result<(), Error> {
        self.conn_tx()?.send_only(Login {
            namespace: "email".to_string(),
            id: email,
            password,
        });
        Ok(())
    }

    pub fn logout(&self) -> Result<(), Error> {
        self.conn_tx()?.send_only(Logout);
        Ok(())
    }
}
