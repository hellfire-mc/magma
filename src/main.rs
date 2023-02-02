mod bridge;
mod config;
mod cryptor;
mod io;
mod protocol;
mod proxy;

use std::{env, path::PathBuf};

use ansi_term::{Color, Style};
use anyhow::{Context, Result};
use clap::Parser;
use config::Config;
use futures::future;
use io::ProtocolReadExt;

use proxy::ProxyServer;
use tokio::{fs::write, join};
use tracing::{debug, info};
use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

/// Magam is a light-weight domain-switching reverse proxy for Minecraft servers.
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
                .with_default_directive("magma=info".parse().unwrap())
                .from_env()
                .context("Failed to parse RUST_LOG environment variable")
                .unwrap(),
        )
        .init();
    // splash!
    println!(
        "\n{} v{} ({})",
        Style::new().bold().paint("magma"),
        env!("CARGO_PKG_VERSION"),
		env!("VERGEN_GIT_SHA_SHORT")
    );
    println!("{}\n", Color::Black.paint("made with 💜 by kaylen"));

    let config = env::current_dir()
        .context("failed to locate current directory")
        .unwrap()
        .join(args.config);
    // ensure config exists
    if !config.exists() {
        debug!("Failed to locate config file - copying defaults...");
        write(config.clone(), include_str!("../assets/config.v1.toml"))
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
    let config = config.build().context("failed to build configuration")?;

    let route_count = config
        .proxies
        .iter()
        .map(|proxy| proxy.routes.len())
        .reduce(|a, b| a + b)
        .unwrap_or(0);

    info!(
        "Loaded {} proxy configuration(s) with {} route(s)",
        config.proxies.len(),
        route_count
    );

    let mut handles = vec![];
    for config in config.proxies {
        let proxy = ProxyServer::from_config(config).context("failed to create proxy server")?;
        handles.push(proxy.spawn());
    }

    future::join_all(handles).await;

    Ok(())
}
