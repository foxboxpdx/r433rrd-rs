use std::{env, fs};
use std::error::Error;
use tokio::net::UdpSocket;
use r433rrd_rs::{Server, ConfigFile};
use log::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let conffile = env::args().nth(1).unwrap_or("r433rrd.conf".to_string());
    let confstr = fs::read_to_string(conffile).expect("Couldn't read config");
    let config: ConfigFile = toml::from_str(&confstr).expect("Couldn't parse config");

    let socket = UdpSocket::bind(config.listen_addr).await?;

    info!("Relay service starting up...");
    let server = Server { socket, buf: vec![0; 1024], to_send: None, config };
    server.run().await?;
    Ok(())
}
