//! Contains a basic Minecraft server for handling incoming clients.

use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Error, Result};

use mc_chat::TextComponent;

use rand::{thread_rng, Rng};

use tokio::{net::TcpListener, task::JoinHandle};
use tracing::{debug, warn, span, Level, info};

use crate::bridge::Bridge;

/// A proxy server.
pub struct ProxyServer {
    config: ProxyServerConfig,
    clients: Vec<Bridge>,
}

/// The internal configuration definition.
#[derive(Debug)]
pub struct MagmaConfig {
    /// Whether to enable debug logging.
    pub debug: bool,
    /// A list of proxy servers.
    pub proxies: Vec<ProxyServerConfig>,
}

/// The configuration for a proxy server.
#[derive(Debug)]
pub struct ProxyServerConfig {
    /// The binding address of the server.
    pub listen_addr: SocketAddr,
    /// A list of routes this server uses.
    pub routes: Vec<ProxyServerRoute>,
    /// The fallback method this server uses.
    pub fallback_method: FallbackMethod,
}

impl Default for ProxyServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:25565".parse().unwrap(),
            routes: Vec::new(),
            fallback_method: FallbackMethod::default(),
        }
    }
}

/// A server route configuration.
#[derive(Debug)]
pub struct ProxyServerRoute {
    /// Where the server should accept connections from.
    pub from: String,
    /// Where the server should proxy connections to.
    pub to: Vec<SocketAddr>,
    /// The selection algorithm to use.
    pub selection_algorithm: SelectionAlgorithmKind,
}

#[derive(Default, Debug)]
pub enum FallbackMethod {
    /// Drop the connection.
    #[default]
    Drop,
    /// Return a status message to the client.
    Status(TextComponent),
}

/// The server selection algorithm.
#[derive(Default, Debug)]
pub enum SelectionAlgorithmKind {
    Random,
    #[default]
    RoundRobin,
}

trait SelectionAlgorithm {
    /// Initialize the algorithm.
    fn new(targets: Vec<SocketAddr>) -> Self;
    /// The kind of algorithm this implements.
    fn kind(&self) -> SelectionAlgorithmKind;
    /// Compute the next target.
    fn next_target(&mut self) -> SocketAddr;
}

struct RoundRobinSelector {
    targets: Vec<SocketAddr>,
    index: usize,
}

impl SelectionAlgorithm for RoundRobinSelector {
    fn new(targets: Vec<SocketAddr>) -> Self {
        Self { targets, index: 0 }
    }

    fn kind(&self) -> SelectionAlgorithmKind {
        SelectionAlgorithmKind::RoundRobin
    }

    fn next_target(&mut self) -> SocketAddr {
        let target = self.targets[self.index];
        self.index = (self.index + 1) % self.targets.len();
        target
    }
}

struct RandomSelector {
    targets: Vec<SocketAddr>,
}

impl SelectionAlgorithm for RandomSelector {
    fn new(targets: Vec<SocketAddr>) -> Self {
        Self { targets }
    }

    fn kind(&self) -> SelectionAlgorithmKind {
        SelectionAlgorithmKind::Random
    }

    fn next_target(&mut self) -> SocketAddr {
        let idx = thread_rng().gen_range(0..self.targets.len());
        self.targets[idx]
    }
}

impl ProxyServer {
    pub fn from_config(config: ProxyServerConfig) -> Result<Self> {
        Ok(Self {
            config,
            clients: Vec::new(),
        })
    }

    /// Consume this server instance and spawn a Tokio task that handles connections.
    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::task::spawn(async move {

            let mut remaining = 6;
			
            loop {
				let span = span!(Level::INFO, "proxy", address=self.config.listen_addr.clone().to_string());
				let _guard = span.enter();

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
                        warn!("Magma will now attempt to restart this server... attempts remaining: {}", remaining);
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

		info!("Successfully started proxy server");

        loop {
            let (_stream, remote_addr) = listener
                .accept()
                .await
                .context("failed to accept new connection")?;
            debug!("New connection from {:?}", remote_addr);
        }
    }
}
