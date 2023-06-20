use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub enum Message {
    Ping,
    Pong,
    Consume { topic: String },
    Publish { topic: String, message: String },
    Deliver { topic: String, message: String },
}
