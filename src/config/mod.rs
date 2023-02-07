mod v1;

use std::{net::SocketAddr, path::Path};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use mc_chat::TextComponent;
use serde::Deserialize;
use tokio::fs::read_to_string;
use tracing::Value;

use self::v1::ConfigV1;

/// The internal configuration definition.
#[derive(Debug)]
pub struct MagmaConfig {
    /// Whether to enable debug logging.
    pub debug: bool,
    /// A list of proxy servers.
    pub proxies: Vec<Proxy>,
}

/// The configuration for a proxy server.
#[derive(Debug)]
pub struct Proxy {
    /// The protocol version to broadcast.
    pub protocol_version: usize,
    /// The binding address of the server.
    pub listen_addr: SocketAddr,
    /// A list of routes this server uses.
    pub routes: Vec<Route>,
    /// The fallback method this server uses.
    pub fallback_method: FallbackMethod,
}

impl Default for Proxy {
    fn default() -> Self {
        Self {
            protocol_version: 761,
            listen_addr: "127.0.0.1:25565".parse().unwrap(),
            routes: Vec::new(),
            fallback_method: FallbackMethod::default(),
        }
    }
}

/// A server route configuration.
#[derive(Debug)]
pub struct Route {
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

/// The latest configuration version.
static LATEST_CONFIG_VERSION: u8 = 1;

pub async fn from_path<P>(path: P) -> Result<impl Config>
where
    P: AsRef<Path>,
{
    let buf = read_to_string(path.as_ref())
        .await
        .context("Failed to read configuration file")?;

    let config: VersionedConfig = toml::from_str(&buf).context("Failed to parse configuration")?;
    match config.version {
        1 => toml::from_str::<ConfigV1>(&buf).context("Failed to parse configuration"),
        _ => bail!("Unknown config version: {}", config.version),
    }
}

#[async_trait]
pub trait Config {
    /// Test if this configuration is of the latest version.
    fn is_latest(&self) -> bool;
    /// Build this configuration into a list of proxy configurations.
    fn build(self) -> Result<MagmaConfig>;
}

#[derive(Deserialize)]
struct VersionedConfig {
    version: u8,
}
