//! Defines the proxy server, and selection algorithms for routing.
//!
//! Magma is capable of proxying connections to multiple servers, by creating a proxy server for each
//! listening address. Each proxy server can have multiple routes, which define where the proxy server
//! should route connections to.

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;

use rand::{thread_rng, Rng};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tracing::{error, info, trace, warn};

use crate::{
    bridge,
    config::{Proxy, SelectionAlgorithmKind},
    io::{ProcotolAsyncWriteExt, ProtocolAsyncReadExt},
    protocol::ProtocolState,
};

/// A selection algorithm for routing new connections to upstream servers.
///
/// Once a connection is established, Magma has to decide which upstream server to route the connection to.
/// This is done by selecting a target from a list of targets using a selection algorithm.
///
/// Magma currently supports two selection algorithms:
/// - [RoundRobinSelector]: This algorithm will select the next target in the list of targets.
/// - [RandomSelector]: This algorithm will select a random target from the list of targets.
pub trait SelectionAlgorithm {
    /// Initialise the selection algorithm with a list of targets it can choose from.
    fn new(targets: Vec<SocketAddr>) -> Self;
    /// The kind of algorithm this implements.
    fn kind(&self) -> SelectionAlgorithmKind;
    /// Compute the next target.
    fn next_target(&mut self) -> SocketAddr;
}

/// A round-robin selection algorithm.
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

/// A random selection algorithm.
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

/// Spawns a new proxy server, and returns a handle to the task.
pub fn spawn(proxy: Proxy) -> JoinHandle<Result<()>> {
    tokio::task::spawn(async move { listen(proxy).await })
}

/// Listen for new connections.
///
/// This function will listen for new connections, and invoke [handle_connection] for each new connection.
#[tracing::instrument(name="proxy", skip_all, fields(addr=%proxy.listen_addr))]
async fn listen(proxy: Proxy) -> Result<()> {
    // create tcp listener
    let listener = TcpListener::bind(proxy.listen_addr).await.map_err(|err| {
        error!("Error while starting proxy server: {}", err);
        err
    })?;
    let proxy = Arc::new(proxy);

    info!("Started proxy server");

    loop {
        // accept new connections, and create a new task for each
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        tokio::task::spawn(handle_connection(proxy.clone(), stream));
    }
}

/// Handle a new connection from a client.
async fn handle_connection(proxy: Arc<Proxy>, mut client_stream: TcpStream) -> Result<()> {
    // read the first packet from the client - this should be a handshake packet
    let handshake = client_stream.read_uncompressed_packet().await?;
    if handshake.id != 0x00 {
        trace!("Received unexpected packet from client: {:?}", handshake.id);
        client_stream.shutdown().await?;
    }
    // read target server address
    let mut handshake = handshake.as_cursor();
    let protocol_version = handshake.read_var_int().await?;
    let server_address = handshake.read_string().await?;
    let _ = handshake.read_u16().await?;
    let next_state: ProtocolState = handshake.read_var_int().await?.try_into()?;

    // lookup target server
    let target = proxy.routes.iter().find(|r| r.from == server_address);
    if target.is_none() {
        warn!("No target server found for address: {}", server_address);
        client_stream.shutdown().await?;
        return Ok(());
    }
    let target = &target.unwrap().to[rand::thread_rng().gen_range(0..target.unwrap().to.len())];

    // create a new connection to the target server
    let mut server_stream = TcpStream::connect(target).await?;

    // write handshake packet to server
    server_stream.write_var_int(0x00).await?;
    server_stream.write_var_int(protocol_version).await?;
    server_stream
        .write_string(proxy.listen_addr.ip().to_string())
        .await?;
    server_stream.write_u16(proxy.listen_addr.port()).await?;
    server_stream.write_var_int((&next_state).into()).await?;

    // create bridge
    bridge::create(next_state, client_stream, server_stream).await
}
