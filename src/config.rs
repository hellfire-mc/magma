use std::net::{IpAddr, SocketAddr};

use serde::Deserialize;

/// The Moss configuration object.
#[derive(Deserialize)]
pub struct Config {
    pub version: u8,
    pub listener: ListenerConfig,
    pub servers: Vec<ServerEntry>,
}

/// The proxy's TCP listener configurationn.
#[derive(Deserialize)]
pub struct ListenerConfig {
    pub bind_address: IpAddr,
    pub port: u16,
}

/// A server entry block.
#[derive(Deserialize)]
pub struct ServerEntry {
    /// The server domain.
    pub domain: Option<String>,
    /// A list of valid server domains.
    #[serde(default = "Vec::new")]
    pub domains: Vec<String>,
    pub target: Option<SocketAddr>,
    #[serde(default = "Vec::new")]
    pub targets: Vec<SocketAddr>,
}

/// The server selection algorithm.
#[derive(Deserialize)]
pub enum SelectionAlgorithm {
    #[serde(rename = "random")]
    Random,
    #[serde(rename = "round-robin")]
    RoundRobin,
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn test_default_config() {
        let _: Config = toml::from_str(include_str!("../assets/config.toml")).unwrap();
    }
}
