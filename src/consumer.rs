use futures::{SinkExt, TryStreamExt};
use messages::Message;
use serde_json::json;
use tokio_serde::formats::SymmetricalJson;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

mod messages;

#[tokio::main]
async fn main() {
    let socket = tokio::net::TcpStream::connect("localhost:1234")
        .await
        .unwrap();

    let length_delimited = Framed::new(socket, LengthDelimitedCodec::new());
    let mut bidirectional =
        tokio_serde::SymmetricallyFramed::new(length_delimited, SymmetricalJson::default());

    bidirectional.send(json!(Message::Ping)).await.unwrap();
    bidirectional
        .send(json!(Message::Consume {
            topic: String::from("topic")
        }))
        .await
        .unwrap();

    loop {
        match bidirectional.try_next().await {
            Ok(Some(message)) => {
                println!("[+] Received message: {:?}.", message);
            }
            Ok(None) => {
                println!("[+] Connection closed.");
                break;
            }
            Err(err) => {
                println!("[-] Error receiving message: {}.", err);
            }
        }
    }
}
