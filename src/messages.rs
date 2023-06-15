use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub enum Message {
    Ping,
    Consume { topic: String },
    Publish { topic: String, message: String },
    Pong,
    Deliver { topic: String, message: String },
}
