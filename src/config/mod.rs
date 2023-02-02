mod v1;

use std::path::Path;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use tokio::fs::read_to_string;

use crate::proxy::MagmaConfig;

use self::v1::ConfigV1;

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
