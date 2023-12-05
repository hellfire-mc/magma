//! Defines the bridge between the client and server.
//!
//! Once a client has connected to the proxy, the proxy will attempt to connect to the upstream,
//! and if successful, will create a bridge to proxy data between the two streams.

use std::sync::Arc;

use anyhow::Result;
use tokio::{net::TcpStream, sync::RwLock, try_join};
use tracing::debug;

use crate::{
    bridge::{downstream::handle_downstream, upstream::handle_upstream},
    cryptor::Cryptor,
    protocol::ProtocolState,
};

mod downstream;
mod upstream;

/// Stores the state of a bridge, comprised of the protocol state of the client and server.
pub struct BridgeState {
    /// The state of the client connection.
    pub client: RwLock<ClientState>,
    /// The state of the server connection.
    pub server: RwLock<ServerState>,
}

/// Stores the state of a client connection.
pub struct ClientState {
    /// The protocol state.
    protocol_state: ProtocolState,
    /// Whether the connection is compressed.
    compressed: bool,
    /// The connection cryptor.
    cryptor: Cryptor,
}

/// Stores the state of a server connection.
pub struct ServerState {
    /// The protocol state.
    protocol_state: ProtocolState,
    /// Whether the connection is compressed.
    compressed: bool,
}

impl From<ProtocolState> for BridgeState {
    fn from(state: ProtocolState) -> Self {
        Self {
            client: RwLock::new(ClientState {
                protocol_state: state.clone(),
                compressed: false,
                cryptor: Cryptor::Uninitialized,
            }),
            server: RwLock::new(ServerState {
                protocol_state: state,
                compressed: false,
            }),
        }
    }
}

/// Consume the provided streams and bridge data between them.
#[tracing::instrument(skip_all, name = "bridge", fields(server_addr))]
pub async fn create(
    state: ProtocolState,
    client_stream: TcpStream,
    server_stream: TcpStream,
) -> Result<()> {
    // create state
    let state = Arc::new(BridgeState::from(state));

    // split streams
    let (client_rx, client_tx) = client_stream.into_split();
    let (server_rx, server_tx) = server_stream.into_split();

    // spawn upstream and downstream tasks
    let upstream = tokio::task::spawn(handle_upstream(state.clone(), client_rx, server_tx));
    let downstream = tokio::task::spawn(handle_downstream(state.clone(), server_rx, client_tx));

    debug!("Bridge initialized");

    // wait for either task to finish
    try_join!(upstream, downstream)
        .map(|_| ())
        .map_err(|e| e.into())
}
