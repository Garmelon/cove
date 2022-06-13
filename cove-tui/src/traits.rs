use async_trait::async_trait;
use chrono::{DateTime, Utc};

pub trait Msg {
    type Id;
    fn id(&self) -> Self::Id;

    fn time(&self) -> DateTime<Utc>;
    fn nick(&self) -> String;
    fn content(&self) -> String;
}

#[async_trait]
pub trait MsgStore<M: Msg> {
    async fn path(room: &str, id: M::Id) -> Vec<M::Id>;
}
