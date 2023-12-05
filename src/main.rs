//! Magma is a light-weight domain-switching reverse proxy for Minecraft servers.
//!
//! # Features
//!
//! - **Light-weight**: Magma is designed to be as light-weight as possible, and uses minimal resources.
//! - **Fast**: Magma is written in Rust, and is designed to be fast.
//! - **Secure**: Magma supports the Minecraft protocol encryption, and uses it by default.
//! - **Flexible**: Magma supports multiple routing algorithms, and can be configured to use any of them.
//! - **Easy to use**: Magma is easy to use, and can be configured using a simple TOML configuration file.

use std::{env, path::PathBuf};

use ansi_term::{Color, Style};
use anyhow::{Context, Result};
use clap::Parser;
use futures::future::try_join_all;
use time::macros::format_description;
use tokio::fs::write;
use tracing::{debug, error, info};
use tracing_subscriber::{
    fmt::{self, time::UtcTime},
    prelude::*,
    util::SubscriberInitExt,
    EnvFilter,
};

mod bridge;
mod config;
mod cryptor;
mod io;
mod proxy;

use config::Config;

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
        .with(fmt::layer().with_timer(UtcTime::new(format_description!(
            "[hour]:[minute]:[second]"
        ))))
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
        env!("VERGEN_GIT_SHA")
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
    info!("Loading configuration from {:?}...", config);
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
        handles.push(proxy::spawn(config));
    }

    match try_join_all(handles).await {
        Ok(errs) => {
            let errs = errs.iter().filter(|r| r.is_err()).count();
            if errs != 0 {
                error!("Encountered an unrecoverable error - Magma will now exit")
            }
            Ok(())
        }
        Err(err) => {
            error!("Encountered error while starting proxies: {}", err);
            Ok(())
        }
    }
}
