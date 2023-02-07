//! Contains a basic Minecraft server for handling incoming clients.

use std::{net::SocketAddr, sync::Arc};

use anyhow::{bail, Result};
use mc_chat::{ChatComponent, ComponentStyle, TextComponent};
use rand::{thread_rng, Rng};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tracing::{debug, error, info, warn};

use crate::{
    bridge,
    config::{FallbackMethod, Proxy, SelectionAlgorithmKind},
    io::{ProcotolWriteExt, ProtocolReadExt},
    protocol::StatusResponse,
};

pub trait SelectionAlgorithm {
    /// Initialize the algorithm.
    fn new(targets: Vec<SocketAddr>) -> Self;
    /// The kind of algorithm this implements.
    fn kind(&self) -> SelectionAlgorithmKind;
    /// Compute the next target.
    fn next_target(&mut self) -> SocketAddr;
}

pub struct RoundRobinSelector {
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

pub struct RandomSelector {
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

/// Spawn a proxy without blocking.
pub fn spawn(proxy: Proxy) -> JoinHandle<Result<()>> {
    tokio::task::spawn(async move { listen(proxy).await })
}

/// Create a listener and listen for incoming packets.
#[tracing::instrument(name="proxy", skip_all, fields(addr=%proxy.listen_addr))]
pub async fn listen(proxy: Proxy) -> Result<()> {
    let listener = TcpListener::bind(proxy.listen_addr).await.map_err(|err| {
        error!("Error while starting proxy server: {}", err);
        err
    })?;
    let proxy = Arc::new(proxy);

    info!("Started proxy server");

    loop {
        let (client_stream, client_addr) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let proxy = proxy.clone();
        tokio::task::spawn(async move {
            if let Err(e) = handle(proxy.as_ref(), client_stream, client_addr).await {
                warn!("Encountered an error while handling packets: {}", e);
            }
        });
    }
}

/// Handle a connection from a client.
#[tracing::instrument(name="proxy", skip_all, fields(addr=%proxy.listen_addr))]
async fn handle(
    proxy: &Proxy,
    mut client_stream: TcpStream,
    mut client_addr: SocketAddr,
) -> Result<()> {
    info!("New connection from {:?}", client_addr);
    let handshake = handshake(&proxy, &mut client_stream).await?;
    // locate route
    let route = proxy
        .routes
        .iter()
        .find(|r| r.from == handshake.server_address);
    let route = match route {
        Some(route) => route,
        None => {
            debug!(
                "Server address '{}' did not match any configured routes",
                handshake.server_address
            );
            return match &proxy.fallback_method {
                FallbackMethod::Drop => {
                    debug!("Dropping connection...");
                    Ok(())
                }
                FallbackMethod::Status(status) => {
                    debug!("Sending status to client...");
                    handle_status_fallback(client_stream, status).await
                }
            };
        }
    };
    // TODO: proper route selection
    let server_addr = route.to.first().unwrap();
    info!("Bridging client at {} to {}", client_addr, server_addr);

    match handshake.next_state {
        1 => handle_upstream_status(client_stream, *server_addr, handshake.protocol_version).await,
        2 => {
            handle_upstream_login(
                client_addr,
                client_stream,
                *server_addr,
                handshake.protocol_version,
            )
            .await
        }
        _ => bail!("illegal next state"),
    }
}

pub struct HandshakeResult {
    next_state: u8,
    server_address: String,
    server_port: u16,
    protocol_version: u16,
}

#[tracing::instrument(skip_all)]
async fn handshake(proxy: &Proxy, client_stream: &mut TcpStream) -> Result<HandshakeResult> {
    let packet = client_stream.read_packet().await?;
    let mut packet = packet.as_cursor();

    let protocol_version = packet.read_var_int().await? as u16;
    let server_address = packet.read_string().await?;
    let server_port = packet.read_u16().await?;
    let next_state = packet.read_var_int().await? as u8;

    debug!(
        protocol_version,
        server_address, server_port, next_state, "Handshake successful"
    );

    Ok(HandshakeResult {
        protocol_version,
        server_address,
        next_state,
        server_port,
    })
}

#[tracing::instrument(name="status", skip_all)]
async fn handle_status_fallback(
    mut client_stream: TcpStream,
    status: &TextComponent,
) -> Result<()> {
    let mut buf = vec![];
	let message = serde_json::to_string(&status)?;
	buf.write_string(message).await?;
	client_stream.write_packet(0x00, &buf).await?;
	Ok(())
}

/// Connect to the upstream server and return its status response.
#[tracing::instrument(name="status", skip_all)]
async fn handle_upstream_status(
    mut client_stream: TcpStream,
    server_addr: SocketAddr,
    protocol_version: u16,
) -> Result<()> {
    let mut server_stream = match TcpStream::connect(server_addr).await {
        Ok(s) => s,
        Err(_) => {
            warn!("Failed to connect to upstream server: {}", server_addr);
            let status = serde_json::to_string(&StatusResponse::message(
                "Failed to connect to upstream server",
            ))?;

            loop {
                let packet = client_stream.read_packet().await?;
                match packet.id {
                    0x00 => {
                        let mut buf = vec![];
                        buf.write_string(status).await?;
                        client_stream.write_packet(0x00, &buf).await?;
                    }
                    0x01 => {
                        let mut buf = vec![];
                        buf.write_u64(packet.as_cursor().read_u64().await?).await?;
                        client_stream.write_packet(0x01, &buf).await?;
                        continue;
                    }
                    _ => bail!("illegal packet id"),
                }

                return Ok(());
            }
        }
    };

    // write handshake packet
    debug!("Writing handshake packet...");
    let mut buf = vec![];
    buf.write_var_int(protocol_version as i32).await?;
    buf.write_string(server_addr.ip().to_string()).await?;
    buf.write_u16(server_addr.port()).await?;
    buf.write_var_int(1).await?;
    server_stream.write_packet(0x00, &buf).await?;

    // handle status packets from the client
    loop {
        let packet = client_stream.read_packet().await?;
        server_stream
            .write_packet(packet.id.try_into().unwrap(), &packet.data)
            .await?;
        let response = server_stream.read_packet().await?;
        client_stream
            .write_packet(packet.id.try_into().unwrap(), &response.data)
            .await?;
    }
}

/// Connect to the upstream server and attempt to
#[tracing::instrument(name="login", skip_all)]
async fn handle_upstream_login(
    client_addr: SocketAddr,
    mut client_stream: TcpStream,
    server_addr: SocketAddr,
    protocol_version: u16,
) -> Result<()> {
    let server_stream = match TcpStream::connect(server_addr).await {
        Ok(s) => s,
        Err(_) => {
            warn!("Failed to connect to upstream server: {}", server_addr);
            let _packet = client_stream.read_packet().await?;
            let mut buf = vec![];
            let message = serde_json::to_string(&ChatComponent::from_text(
                "Failed to connect to upstream server",
                ComponentStyle::v1_16(),
            ))?;
            buf.write_string(message).await?;
            client_stream.write_packet(0x00, &buf).await?;
            return Ok(());
        }
    };

    bridge::create(client_addr, client_stream, server_addr, server_stream).await
}
