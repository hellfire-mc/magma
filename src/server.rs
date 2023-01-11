//! Contains a basic Minecraft server for handling incoming clients.

use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Error};
use mc_chat::TextComponent;
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tracing::{debug, warn};

use crate::client::Client;

pub struct ServerConfig {
    /// The binding address of the server.
    pub listen_addr: SocketAddr,
    /// A list of routes this server uses.
    pub routes: Vec<ServerRoute>,
    /// The fallback method this server uses.
    pub fallback_method: FallbackMethod,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:25565".parse().unwrap(),
            routes: Vec::new(),
            fallback_method: FallbackMethod::default(),
        }
    }
}

/// A server route configuration.
pub struct ServerRoute {
    /// Where the server should accept connections from.
    pub from: String,
    /// Where the server should proxy connections to.
    pub to: SocketAddr,
}

#[derive(Default)]
pub enum FallbackMethod {
    /// Drop the connection.
    #[default]
    Drop,
    /// Return a status message to the client.
    Status(TextComponent),
}

/// A proxy server.
pub struct Server {
    config: ServerConfig,
    clients: Vec<ServerClient>,
}

impl Server {
    /// Consume this server instance and spawn a Tokio task that handles connections.
    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::task::spawn(async move {
            let mut remaining = 5;
            loop {
                // decrement remaining starts
                remaining -= 1;
                // start listening
                match self.listen().await {
                    Ok(()) => {
                        debug!("Server gracefully shutdown");
                        break;
                    }
                    Err(err) => {
                        warn!("Server encountered an unrecoverable error: {}", err);
                        // don't restart if failed
                        if remaining == 0 {
                            warn!("Server has reached its maximum allowed restarts - shutdown permanent");
                            break;
                        }
                        // restart server
                        warn!("Moss will now attempt to restart this server... attempts remaining: {}", remaining);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        })
    }

    /// Bind this server to the listen address and start handling connections.
    pub async fn listen(&mut self) -> Result<(), Error> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .context("failed to bind listener")?;

        loop {
            let (stream, remote_addr) = listener
                .accept()
                .await
                .context("failed to accept new connection")?;
            debug!("New connection from {:?}", remote_addr);
        }
    }
}

/// A client connected to a server instance.
enum ServerClient {
    Partial {
        remote_addr: SocketAddr,
        remote_stream: TcpStream,
    },
    Proxied {
        remote_addr: SocketAddr,
        remote_stream: TcpStream,
        proxy_target: SocketAddr,
        proxy_client: Client,
    },
}
