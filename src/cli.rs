use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CmdArgs {
    #[arg(short, long, default_value_t = String::from("config.toml"))]
    pub config_path: String,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub server: ServerConfig,
}

#[derive(Deserialize, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub message_buffer_size: usize,
}

impl Config {
    pub fn get_socket_address(&self) -> String {
        return format!("{}:{}", self.server.host, self.server.port);
    }
}
