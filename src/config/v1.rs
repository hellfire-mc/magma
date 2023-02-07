use std::{collections::HashMap, net::SocketAddr};

use anyhow::Result;
use serde::Deserialize;
use tracing::warn;

use super::{Config, FallbackMethod, MagmaConfig, Proxy, Route, SelectionAlgorithmKind};

/// The Moss configuration object.
#[derive(Deserialize)]
pub struct ConfigV1 {
    /// The configuration version.
    pub version: u8,
    /// The debug version.
    pub debug: bool,
    /// A list of server entries.
    pub proxies: Vec<ProxyEntry>,
}

/// A server entry block.
#[derive(Deserialize)]
pub struct ProxyEntry {
    /// The proxy listening address.
    pub address: Option<SocketAddr>,
    /// A list of addresses to listen on.
    #[serde(default = "Vec::new")]
    pub addresses: Vec<SocketAddr>,
    /// The proxy domain.
    pub domain: Option<String>,
    /// A list of valid domains.
    #[serde(default = "Vec::new")]
    pub domains: Vec<String>,
    /// The target of this proxy
    pub target: Option<SocketAddr>,
    #[serde(default = "Vec::new")]
    /// A list of target servers.
    pub targets: Vec<SocketAddr>,
    /// The selection algorithm to use.
    pub selection_algorithm: Option<SelectionAlgorithm>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SelectionAlgorithm {
    /// Pick a random target.
    Random,
    #[default]
    /// Pick the next target.
    RoundRobin,
}

impl Config for ConfigV1 {
    fn is_latest(&self) -> bool {
        true
    }

    fn build(self) -> Result<MagmaConfig> {
        let mut proxies: HashMap<SocketAddr, Proxy> = HashMap::new();

        for (i, proxy) in self.proxies.into_iter().enumerate() {
            let addresses = proxy
                .address
                .map(|addr| vec![addr])
                .unwrap_or_else(|| proxy.addresses);
            if addresses.is_empty() {
                warn!("Proxy entry {} for domain(s) {:?} did not provide any addresses or ports to bind to - it will be ignored", i, proxy.domains);
                continue;
            }

            for address in addresses {
                // collect domains
                let domains = proxy
                    .domain
                    .clone()
                    .map(|domain| vec![domain])
                    .unwrap_or_else(|| proxy.domains.clone());
                // ignore empty domains
                if domains.is_empty() {
                    warn!(
                        "Proxy entry {} does not specify any domains - it will be ignored",
                        i
                    );
                    continue;
                }
                // compute targets
                let targets = proxy
                    .target
                    .map(|target| vec![target])
                    .unwrap_or_else(|| proxy.targets.clone());
                // ignore empty targets
                if targets.is_empty() {
                    warn!(
                        "Proxy entry {} does not specify any targets - it will be ignored",
                        i
                    );
                    continue;
                }

                // build routes
                let mut routes: Vec<_> = domains
                    .iter()
                    .map(|domain| Route {
                        from: domain.clone(),
                        to: targets.clone(),
                        selection_algorithm: proxy
                            .selection_algorithm
                            .clone()
                            .map(|s| match s {
                                SelectionAlgorithm::Random => SelectionAlgorithmKind::Random,
                                SelectionAlgorithm::RoundRobin => {
                                    SelectionAlgorithmKind::RoundRobin
                                }
                            })
                            .unwrap_or_default(),
                    })
                    .collect();

                match proxies.get_mut(&address) {
                    Some(entry) => {
                        // ensure we are not about to overrite existing domains
                        if entry
                            .routes
                            .iter()
                            .any(|route| domains.contains(&route.from))
                        {
                            warn!("The domain(s) {:?} have already been specified for use in another proxy", domains);
                            continue;
                        };

                        entry.routes.append(&mut routes)
                    }
                    None => {
                        proxies.insert(
                            address,
                            Proxy {
                                protocol_version: 761,
                                listen_addr: address,
                                fallback_method: FallbackMethod::Drop,
                                routes,
                            },
                        );
                    }
                }
            }
        }

        Ok(MagmaConfig {
            debug: self.debug,
            proxies: proxies.into_values().collect(),
        })
    }
}
