mod client;
mod config;
mod cryptor;
mod io;
mod protocol;
mod server;

use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use io::ProtocolReadExt;
use rsa::{RsaPrivateKey, RsaPublicKey};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info, trace};
use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

use crate::io::ProcotolWriteExt;

pub struct ClientEncryption {
    pub public_key: RsaPublicKey,
    pub private_key: RsaPrivateKey,
}

/// A bridge between a client and a server.
pub struct Bridge {
    pub client: TcpStream,
    pub client_addr: SocketAddr,
    pub client_encryption: ClientEncryption,
    pub server: TcpStream,
    pub server_addr: SocketAddr,
}

async fn handle_connection(stream: &mut TcpStream, addr: SocketAddr) -> Result<()> {
    // handshaking state
    let _ = stream
        .read_var_int()
        .await
        .context("failed to read packet length")?;
    let packet_id = stream
        .read_var_int()
        .await
        .context("failed to read packet id")?;
    // expect packet id
    if packet_id != 0x00 {
        bail!("invalid packet id: {}", packet_id);
    }
    // read handshake packet
    let _ = stream
        .read_var_int()
        .await
        .context("failed to read protocol version")?;
    let server_address = stream
        .read_string()
        .await
        .context("failed to read server address")?;
    let _ = stream
        .read_u16()
        .await
        .context("failed to read server port")?;
    let next_state = stream
        .read_var_int()
        .await
        .context("failed to read next state")?;

    match next_state {
        1 => handle_status(stream).await,
        2 => handle_login(stream).await,
        _ => bail!("unexpected state: {:?}", next_state),
    }
}

#[tracing::instrument]
async fn handle_status(stream: &mut TcpStream) -> Result<()> {
    debug!("Client entered status state");
    loop {
        let _ = stream
            .read_var_int()
            .await
            .context("failed to read packet length")?;
        let packet_id = stream
            .read_var_int()
            .await
            .context("failed to read packet id")?;

        match packet_id {
            0x00 => {
                trace!("writing status packet");
                // write packet id and response to output
                let mut buf: Vec<u8> = Vec::new();
                buf.write_var_int(0x00).await.unwrap();
                buf.write_string(
                    r#"{
    "version": {
        "name": "1.19",
        "protocol": 759
    },
    "players": {
        "max": 100,
        "online": 5,
        "sample": [
            {
                "name": "thinkofdeath",
                "id": "4566e69f-c907-48ee-8d71-d7ba5aa00d20"
            }
        ]
    },
    "description": {
        "text": "Hello world"
    },
    "enforcesSecureChat": false
}"#
                    .to_string(),
                )
                .await
                .unwrap();
                // write packet length and data
                let len = buf.len();
                stream.write_var_int(len as i32).await.unwrap();
                stream.write_all_buf(&mut buf.as_slice()).await.unwrap();
                trace!("done");
            }
            0x01 => {
                trace!("writing ping packet");
                let nonce = stream.read_u64().await.unwrap();
                // write packet id and nonce
                let mut buf: Vec<u8> = Vec::new();
                buf.write_var_int(0x01).await.unwrap();
                buf.write_u64(nonce).await.unwrap();
                // write packet length and data
                let len = buf.len();
                stream.write_var_int(len as i32).await.unwrap();
                stream.write_all_buf(&mut buf.as_slice()).await.unwrap();
            }
            _ => bail!("invalid packet id: {}", packet_id),
        }
    }
}

async fn handle_login(stream: &mut TcpStream) -> Result<()> {
    println!("handling login");
    Ok(())
}

/// Moss is a light-weight reverse proxy for Minecraft servers.
#[derive(Parser)]
struct Args {
    /// The path to the configuration file.
    #[clap(long, default_value = "config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() {
    // parse arguments
    let args = Args::parse();
    // initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive("moss=info".parse().unwrap())
                .from_env()
                .context("Failed to parse RUST_LOG environment variable")
                .unwrap(),
        )
        .init();

    info!("Starting moss proxy v{}", env!("CARGO_PKG_VERSION"));

    // load config
    let config = env::current_dir()
        .context("failed to locate current directory")
        .unwrap()
        .join(args.config);
    debug!("Loading configuration from {:?}...", config);

    let addr: SocketAddr = "127.0.0.1:25565"
        .parse()
        .context("failed to parse socket address")
        .unwrap();

    let listener = TcpListener::bind(addr).await.unwrap();

    info!("Listening on {:?}", addr);

    loop {
        match listener.accept().await {
            Ok((mut stream, addr)) => {
                debug!("New connection from {:?}", addr);
                tokio::task::spawn(async move {
                    let conn_failure = handle_connection(&mut stream, addr).await;
                    let conn_failure = match conn_failure {
                        Err(err) => stream
                            .shutdown()
                            .await
                            .context(err)
                            .context("Failed to shut down connection after error"),
                        Ok(()) => Ok(()),
                    };
                    if let Err(err) = conn_failure {
                        error!("{}", err);
                    }
                });
            }
            Err(err) => {
                println!("{:?}", err);
            }
        }
    }
}
