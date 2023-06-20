mod cli;
mod messages;

use std::net::SocketAddr;

use clap::Parser;
use cli::{CmdArgs, Config};
use futures::TryStreamExt;
use messages::Message;
use tokio::{
    fs::read_to_string,
    net::{TcpListener, TcpStream},
    spawn,
};
use tokio_serde::{
    formats::{Json, SymmetricalJson},
    SymmetricallyFramed,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

async fn listen(address: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).await?;
    println!("[i] Listeing on {}...", address);

    loop {
        match listener.accept().await {
            Ok((client_socket, client_address)) => {
                println!("[+] Accepted client from {}.", client_address);
                spawn(handle_client(client_socket, client_address));
            }
            Err(error) => println!("[!] Could not accept client. Err: {}", error),
        }
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
) -> std::io::Result<()> {
    let length_delimited_framed = Framed::new(client_socket, LengthDelimitedCodec::new());
    let mut bidirectional_framed: BidirectionalFramed = SymmetricallyFramed::new(
        length_delimited_framed,
        SymmetricalJson::<Message>::default(),
    );

    loop {
        match bidirectional_framed.try_next().await {
            Ok(Some(message)) => {
                println!(
                    "[+] Received message from {}: {:?}.",
                    client_address, message
                )
            }
            Ok(None) => {
                println!("[!] Client {} disconnected.", client_address);
                break;
            }
            Err(error) => println!(
                "[!] Invalid message received from {}. Err: {}",
                client_address, error
            ),
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = CmdArgs::parse();
    let config_file = read_to_string(args.config_path).await.unwrap();
    let config: Config = toml::from_str(config_file.as_str()).unwrap();

    listen(&config.get_socket_address()).await.unwrap()
}
