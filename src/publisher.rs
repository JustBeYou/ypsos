use futures::SinkExt;
use messages::Message;
use serde_json::json;
use tokio_serde::formats::SymmetricalJson;
use tokio_util::codec::{FramedWrite, LengthDelimitedCodec};

mod messages;

#[tokio::main]
async fn main() {
    let socket = tokio::net::TcpStream::connect("localhost:1234")
        .await
        .unwrap();

    let length_delimited = FramedWrite::new(socket, LengthDelimitedCodec::new());
    let mut serialized =
        tokio_serde::SymmetricallyFramed::new(length_delimited, SymmetricalJson::default());

    serialized.send(json!(Message::Ping)).await.unwrap();
    serialized
        .send(json!(Message::Publish {
            topic: String::from("topic"),
            message: String::from("some message")
        }))
        .await
        .unwrap();
}
