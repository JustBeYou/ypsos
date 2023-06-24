mod cli;
mod messages;

use std::{collections::HashMap, net::SocketAddr};

use clap::Parser;
use cli::{CmdArgs, Config};
use futures::{future::join_all, SinkExt, TryStreamExt};
use messages::Message;
use tokio::{
    fs::read_to_string,
    net::{TcpListener, TcpStream},
    select, spawn,
    sync::mpsc,
};
use tokio_serde::{
    formats::{Json, SymmetricalJson},
    SymmetricallyFramed,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

enum BrokerCommand {
    Message {
        key: String,
        message: String,
    },
    Consume {
        key: String,
        channel: mpsc::Sender<ClientCommand>,
    },
    Cancel {
        keys: Vec<String>,
    },
}

enum ClientCommand {
    Message(String),
}

async fn listen(address: &str, message_buffer_size: usize) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).await?;
    println!("[i] Listeing on {}...", address);

    let (client_sender, broker_receiver) = mpsc::channel::<BrokerCommand>(message_buffer_size);

    join_all(vec![
        spawn(accept_clients(listener, client_sender, message_buffer_size)),
        spawn(handle_message_brokerage(broker_receiver)),
    ])
    .await;

    println!("[i] Server is shutting down...");
    Ok(())
}

async fn accept_clients(
    listener: TcpListener,
    client_sender: mpsc::Sender<BrokerCommand>,
    message_buffer_size: usize,
) {
    loop {
        match listener.accept().await {
            Ok((client_socket, client_address)) => {
                println!("[+] Accepted client from {}.", client_address);

                let (broker_sender, client_receiver) =
                    mpsc::channel::<ClientCommand>(message_buffer_size);

                spawn(handle_client(
                    client_socket,
                    client_address,
                    client_sender.clone(),
                    broker_sender,
                    client_receiver,
                ));
            }
            Err(error) => println!("[!] Could not accept client. Err: {}", error),
        }
    }
}

async fn handle_message_brokerage(mut broker_receiver: mpsc::Receiver<BrokerCommand>) {
    let mut routing_database = HashMap::<String, mpsc::Sender<ClientCommand>>::new();

    while let Some(message) = broker_receiver.recv().await {
        match message {
            BrokerCommand::Message { key, message } => match routing_database.get(&key) {
                Some(channel) => {
                    let _ = channel.send(ClientCommand::Message(message)).await;
                }
                None => {
                    println!("[!] Could not route message to {}.", key);
                }
            },
            BrokerCommand::Consume { key, channel } => {
                if routing_database.contains_key(&key) {
                    println!(
                        "[!] Could not consume from {}. Key already subscribed.",
                        key
                    );
                } else {
                    routing_database.insert(key, channel);
                }
            }
            BrokerCommand::Cancel { keys } => {
                for key in keys {
                    routing_database.remove(&key);
                }
            }
        };
    }
}

type BidirectionalFramed = tokio_serde::Framed<
    Framed<TcpStream, LengthDelimitedCodec>,
    Message,
    Message,
    Json<Message, Message>,
>;

async fn handle_client(
    client_socket: TcpStream,
    client_address: SocketAddr,
    client_sender: mpsc::Sender<BrokerCommand>,
    broker_sender: mpsc::Sender<ClientCommand>,
    mut client_receiver: mpsc::Receiver<ClientCommand>,
) -> std::io::Result<()> {
    let length_delimited_framed = Framed::new(client_socket, LengthDelimitedCodec::new());
    let mut bidirectional_framed: BidirectionalFramed = SymmetricallyFramed::new(
        length_delimited_framed,
        SymmetricalJson::<Message>::default(),
    );
    let mut subscriptions: Vec<String> = vec![];

    loop {
        select! {
            client_receiver_event = client_receiver.recv() => {
                match client_receiver_event {
                    Some(message) => match message {
                        ClientCommand::Message(content) => {
                            let result = bidirectional_framed.send(Message::Deliver { topic: String::from("none"), message: content }).await;
                            if result.is_err() {
                                println!("[!] Failed to send message to client {}.", client_address);
                            }
                        }
                    },
                    None => {
                        println!("[!] Client receiver closed for {}. Will disconnect.", client_address);
                        break;
                    }
                };
            }

            socket_event = bidirectional_framed.try_next() => {
                match socket_event {
                    Ok(Some(message)) => {
                        println!(
                            "[+] Received message from {}: {:?}.",
                            client_address, message
                        );

                        match message {
                            Message::Consume { topic } => {
                                subscriptions.push(topic.clone());
                                let _ = client_sender
                                    .send(BrokerCommand::Consume {
                                        key: topic,
                                        channel: broker_sender.clone(),
                                    })
                                    .await;
                            }
                            Message::Publish { topic, message } => {
                                let _ = client_sender
                                    .send(BrokerCommand::Message {
                                        key: topic,
                                        message: message,
                                    })
                                    .await;
                            }
                            _ => {}
                        }
                    }

                    Ok(None) => {
                        println!("[!] Client {} disconnected.", client_address);
                        let _ = client_sender
                            .send(BrokerCommand::Cancel {
                                keys: subscriptions,
                            })
                            .await;
                        break;
                    }

                    Err(error) => println!(
                        "[!] Invalid message received from {}. Err: {}",
                        client_address, error
                    ),
                }
            }

            else => {
                println!("[!] All stream failed for client {}. Will disconnect.", client_address)
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = CmdArgs::parse();
    let config_file = read_to_string(args.config_path).await.unwrap();
    let config: Config = toml::from_str(config_file.as_str()).unwrap();

    listen(
        &config.get_socket_address(),
        config.server.message_buffer_size,
    )
    .await
    .unwrap()
}
