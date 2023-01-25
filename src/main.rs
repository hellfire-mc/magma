mod bridge;
mod config;
mod cryptor;
mod io;
mod protocol;
mod proxy;

use std::{env, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use config::Config;
use io::ProtocolReadExt;

use tokio::fs::write;
use tracing::{debug, info};
use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

/// Moss is a light-weight reverse proxy for Minecraft servers.
#[derive(Parser)]
struct Args {
    /// The path to the configuration file.
    #[clap(long, default_value = "config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
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
    // splash!
    info!("Starting moss proxy v{}", env!("CARGO_PKG_VERSION"));
    let config = env::current_dir()
        .context("failed to locate current directory")
        .unwrap()
        .join(args.config);
    // ensure config exists
    if !config.exists() {
        debug!("Failed to locate config file - copying defaults...");
        write(config.clone(), include_str!("../assets/config.toml"))
            .await
            .context("Failed to write default config file")?;
    }
    // load config
    debug!("Loading configuration from {:?}...", config);
    let config = config::from_path(&config).await?;
    // check config is latest version
    if !config.is_latest() {
        todo!("config migration");
    }

    Ok(())
}
