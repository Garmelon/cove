use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct HelloCmd {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HelloRpl {
    pub msg: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Data {
    HelloCmd(HelloCmd),
    HelloRpl(HelloRpl),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Packet {
    pub id: u64,
    #[serde(flatten)]
    pub data: Data,
}
