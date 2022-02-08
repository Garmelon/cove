use futures::{future, StreamExt, TryStreamExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind(("::0", 40080)).await.unwrap();
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(conn(stream));
    }
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
