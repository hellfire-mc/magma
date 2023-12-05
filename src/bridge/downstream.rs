//! Handles the downstream connection from the server to the client.

use std::sync::Arc;

use anyhow::Result;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::io::{Packet, ProcotolAsyncWriteExt, ProtocolAsyncReadExt};

use super::{BridgeState, ProtocolState};

/// Create a state machine to handle downstream packets - that is, packets from the server to the client.
pub async fn handle_downstream(
    state: Arc<BridgeState>,
    mut server_rx: OwnedReadHalf,
    mut client_tx: OwnedWriteHalf,
) -> Result<()> {
    loop {
        let protocol_state = &{ state.server.read().await }.protocol_state;
        match protocol_state {
            ProtocolState::Handshaking => {
                unreachable!("downstream handshake")
            }
            ProtocolState::Status => {
                handle_downstream_status(&mut server_rx, &mut client_tx).await?
            }
            ProtocolState::Login => {
                handle_downstream_login(state.clone(), &mut server_rx, &mut client_tx).await?
            }
            ProtocolState::Play => {
                handle_downstream_play(state.clone(), &mut server_rx, &mut client_tx).await?
            }
        }
    }
}

/// Handle status packets.
async fn handle_downstream_status(
    server_rx: &mut OwnedReadHalf,
    client_tx: &mut OwnedWriteHalf,
) -> Result<()> {
    let packet = server_rx.read_uncompressed_packet().await?;
    client_tx.write_uncompressed_packet(&packet).await?;
    Ok(())
}

/// Handle login packets.
async fn handle_downstream_login(
    state: Arc<BridgeState>,
    server_rx: &mut OwnedReadHalf,
    client_tx: &mut OwnedWriteHalf,
) -> Result<()> {
    todo!()
}

/// Handle play packets.
async fn handle_downstream_play(
    state: Arc<BridgeState>,
    server_rx: &mut OwnedReadHalf,
    client_tx: &mut OwnedWriteHalf,
) -> Result<()> {
    let packet = match { state.server.read().await }.compressed {
        true => Packet::Compressed(server_rx.read_compressed_packet().await?),
        false => Packet::Uncompressed(server_rx.read_uncompressed_packet().await?),
    };

    todo!("handle client packet encryption")
}
