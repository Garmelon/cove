mod api;

use std::convert::Infallible;

use tokio::sync::{mpsc, oneshot};

pub use api::{Message, SessionView, Snowflake, Time, UserId};

enum Request {}

pub struct EuphRoom {
    canary: oneshot::Sender<Infallible>,
    tx: mpsc::Sender<Request>,
}
