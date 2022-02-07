use cove_core::{Data, HelloCmd, Packet};

fn main() {
    println!("Hello, world!");
    let packet = Packet {
        id: 1337,
        data: Data::HelloCmd(HelloCmd {
            name: "Garmy".to_string(),
        }),
    };
    println!("{}", serde_json::to_string(&packet).unwrap());
}
