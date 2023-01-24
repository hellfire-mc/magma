use std::net::{IpAddr, SocketAddr};

use serde::Deserialize;

/// The Moss configuration object.
#[derive(Deserialize)]
pub struct ConfigV1 {
    /// The configuration version.
    pub version: u8,
    /// The debug version.
    pub debug: bool,
    pub proxies: Vec<ProxyEntry>,
}

/// A server entry block.
#[derive(Deserialize)]
pub struct ProxyEntry {
    /// The server listening address.
    pub listen_addr: IpAddr,
    /// The server domain.
    pub domain: Option<String>,
    /// A list of valid server domains.
    #[serde(default = "Vec::new")]
    pub domains: Vec<String>,
    pub target: Option<SocketAddr>,
    #[serde(default = "Vec::new")]
    pub targets: Vec<SocketAddr>,
}
