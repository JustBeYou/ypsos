mod messages;

use std::{
    collections::HashMap,
    io::Result,
    sync::{Arc, Mutex},
};

use clap::Parser;
use futures::{future::join_all, SinkExt, TryStreamExt};
use messages::Message;
use serde::Deserialize;
use tokio::{
    fs::read_to_string,
    net::{TcpListener, TcpStream},
    spawn,
    sync::mpsc::{self, Sender},
};
use tokio_serde::formats::{Json, SymmetricalJson};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CmdArgs {
    #[arg(short, long, default_value_t = String::from("config.toml"))]
    config_path: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    server: ServerConfig,
}

#[derive(Deserialize, Debug)]
struct ServerConfig {
    host: String,
    port: u16,
}

impl ServerConfig {
    pub fn get_socket_address(&self) -> String {
        return format!("{}:{}", self.host, self.port);
    }
}

type BidirectionalFramed = tokio_serde::Framed<
    Framed<TcpStream, LengthDelimitedCodec>,
    Message,
    Message,
    Json<Message, Message>,
>;

async fn start_server(config: Config) -> Result<()> {
    let server_address = config.server.get_socket_address();
    let listener = TcpListener::bind(server_address.as_str()).await?;
    println!("[+] Listening on {}.", server_address);

    let (write_db_channel, mut read_db_channel) = mpsc::channel::<Command>(32);

    let server_task = spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    println!("[+] Received connection from {}.", addr);

                    let write_db_channel = write_db_channel.clone();
                    spawn(async move {
                        handle_client(write_db_channel, socket).await;
                    });
                }
                Err(err) => {
                    println!("[-] Failed to handle connection: {}.", err);
                }
            }
        }
    });

    let db_task = spawn(async move {
        while let Some(message) = read_db_channel.recv().await {
            todo!();
        }
    });

    join_all(vec![server_task, db_task]).await;

    Ok(())
}

#[derive(Debug)]
enum Command {
    Set { Key: String },
}

async fn handle_client(write_db_channel: Sender<Command>, socket: TcpStream) {
    let length_delimited = Framed::new(socket, LengthDelimitedCodec::new());
    let mut bidirectional: BidirectionalFramed = tokio_serde::SymmetricallyFramed::new(
        length_delimited,
        SymmetricalJson::<Message>::default(),
    );

    loop {
        match bidirectional.try_next().await {
            Ok(Some(message)) => {
                println!("[+] Received message: {:?}.", message);

                let result = match message {
                    Message::Ping => bidirectional.send(Message::Pong).await,
                    Message::Consume { topic } => {
                        write_db_channel
                            .send(Command::Set { Key: topic })
                            .await
                            .unwrap();
                        Ok(())
                    }
                    Message::Publish { topic, message } => Ok(()),
                    Message::Pong => Ok(()),
                    Message::Deliver { topic, message } => Ok(()),
                };

                match result {
                    Ok(_) => {}
                    Err(err) => {
                        println!("Error processing message from connection: {}.", err);
                        return;
                    }
                }
            }
            Ok(None) => {
                println!("[+] Connection closed.");
                return;
            }
            Err(err) => {
                println!("[-] Error receiving message: {}.", err);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args = CmdArgs::parse();
    let config_file = read_to_string(args.config_path).await.unwrap();
    let config: Config = toml::from_str(config_file.as_str()).unwrap();
    start_server(config).await.unwrap()
}
