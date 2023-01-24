mod v1;

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::fs::read_to_string;

use self::v1::ConfigV1;

/// The latest configuration version.
static LATEST_CONFIG_VERSION: u8 = 1;

/// An enumeration of configuration versions.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Config {
    V1(ConfigV1),
}

impl Config {
    /// Read a migratable config from the givne path.
    pub async fn from_path<P: AsRef<Path>>(p: P) -> Result<Self> {
        let config = read_to_string(p.as_ref())
            .await
            .context("Failed to read configuration file")?;
        toml::from_str(&config).context("Failed to parse configuration file")
    }

    /// Test if this configuration is of the latest version.
    pub fn is_latest(&self) -> bool {
        match self {
            Config::V1(_) => true,
        }
    }
}
