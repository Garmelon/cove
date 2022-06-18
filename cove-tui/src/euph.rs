mod api;

use std::convert::Infallible;

use tokio::sync::{mpsc, oneshot};

enum Request {}

pub struct EuphRoom {
    canary: oneshot::Sender<Infallible>,
    tx: mpsc::Sender<Request>,
}
