use cove_core::packets::{
    Cmd, HelloCmd, HelloRpl, JoinNtf, NickCmd, NickNtf, NickRpl, Ntf, Packet, PartNtf, Rpl,
    SendCmd, SendNtf, SendRpl, WhoCmd, WhoRpl,
};
use cove_core::{Identity, Message, MessageId, SessionId, User};
use futures::{future, StreamExt, TryStreamExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    let user = User {
        nick: "Garmy".to_string(),
        identity: Identity::of("random garbage"),
        sid: SessionId::of("12345"),
    };
    let message = Message {
        pred: MessageId::of("pred"),
        parent: None,
        identity: Identity::of("asd"),
        nick: "Foo".to_string(),
        content: "Bar".to_string(),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Cmd {
            id: 12345,
            cmd: Cmd::Hello(HelloCmd {
                nick: "Garmy".to_string(),
                identity: "random garbage".to_string()
            })
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Rpl {
            id: 67890,
            rpl: Rpl::Hello(HelloRpl::Success {
                you: user.clone(),
                others: vec![],
                last_message: MessageId::of("Blarg")
            })
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Rpl {
            id: 67890,
            rpl: Rpl::Hello(HelloRpl::NickTooLong)
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Cmd {
            id: 12345,
            cmd: Cmd::Nick(NickCmd {
                nick: "Garmelon".to_string()
            })
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Rpl {
            id: 67890,
            rpl: Rpl::Nick(NickRpl::Success)
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Rpl {
            id: 67890,
            rpl: Rpl::Nick(NickRpl::NickTooLong)
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Cmd {
            id: 12345,
            cmd: Cmd::Send(SendCmd {
                parent: None,
                // parent: Some(MessageId::of("Booh!")),
                content: "Hello world!".to_string()
            })
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Rpl {
            id: 67890,
            rpl: Rpl::Send(SendRpl::Success {
                message: message.clone()
            })
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Rpl {
            id: 67890,
            rpl: Rpl::Send(SendRpl::ContentTooLong)
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Cmd {
            id: 12345,
            cmd: Cmd::Who(WhoCmd {})
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Rpl {
            id: 67890,
            rpl: Rpl::Who(WhoRpl {
                you: user.clone(),
                others: vec![]
            })
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Ntf {
            ntf: Ntf::Join(JoinNtf { user: user.clone() })
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Ntf {
            ntf: Ntf::Nick(NickNtf { user: user.clone() })
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Ntf {
            ntf: Ntf::Part(PartNtf { user: user.clone() })
        })
        .unwrap()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Packet::Ntf {
            ntf: Ntf::Send(SendNtf {
                message: message.clone()
            })
        })
        .unwrap()
    );
    // let listener = TcpListener::bind(("::0", 40080)).await.unwrap();
    // while let Ok((stream, _)) = listener.accept().await {
    //     tokio::spawn(conn(stream));
    // }
}

async fn conn(stream: TcpStream) {
    println!("Connection from {}", stream.peer_addr().unwrap());
    let stream = tokio_tungstenite::accept_async(stream).await.unwrap();
    let (write, read) = stream.split();
    read.try_filter(|msg| future::ready(msg.is_text() || msg.is_binary()))
        .forward(write)
        .await
        .unwrap();
}
