use cove_core::{HelloCmd, HelloRpl, Id, Packet, Rpl};

fn main() {
    let packet = Packet::Rpl {
        id: 1337,
        rpl: Rpl::Hello(HelloRpl::InvalidName {
            reason: "abc".to_string(),
        }),
    };
    println!("{}", serde_json::to_string_pretty(&packet).unwrap());
}
