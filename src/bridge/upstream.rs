//! Handles the upstream connection from the client to the server.

use std::sync::Arc;

use anyhow::{bail, Result};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};

use crate::{
    io::{ProcotolWriteExt, ProtocolReadExt},
    protocol::ProtocolState,
};

use super::BridgeState;

/// Create a state machine to handle upstream packets - that is, packets from the client to the server.
pub async fn handle_upstream(
    state: Arc<BridgeState>,
    mut client_rx: OwnedReadHalf,
    mut server_tx: OwnedWriteHalf,
) -> Result<()> {
    loop {
        let protocol_state = &{ state.server.read().await }.protocol_state;
        match protocol_state {
            ProtocolState::Handshaking => {
                unreachable!("downstream handshake")
            }
            ProtocolState::Status => handle_upstream_status(&mut client_rx, &mut server_tx).await?,
            ProtocolState::Login => {
                handle_upstream_login(state.clone(), &mut client_rx, &mut server_tx).await?
            }
            ProtocolState::Play => {
                handle_upstream_play(state.clone(), &mut client_rx, &mut server_tx).await?
            }
        }
    }
}

/// Handle status packets.
async fn handle_upstream_status(
    client_rx: &mut OwnedReadHalf,
    server_tx: &mut OwnedWriteHalf,
) -> Result<()> {
    let packet = client_rx.read_uncompressed_packet().await?;
    server_tx.write_uncompressed_packet(&packet).await?;
    Ok(())
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MojangAuthResponse {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "properties")]
    pub properties: Vec<Property>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Property {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "value")]
    pub value: String,
    #[serde(rename = "signature")]
    pub signature: String,
}

/// Handle login packets.
async fn handle_upstream_login(
    state: Arc<BridgeState>,
    client_rx: &mut OwnedReadHalf,
    server_tx: &mut OwnedWriteHalf,
) -> Result<()> {
    // read the login start packet from the client
    let login_start = client_rx.read_uncompressed_packet().await?;
    server_tx.write_uncompressed_packet(&login_start).await?;

    // read login info
    let mut login_start = login_start.as_cursor();
    let username = login_start.read_string().await?;
    let uuid = login_start.read_uuid().await?;

    // read the client encryption response packet
    let encryption_response = client_rx.read_uncompressed_packet().await?;
    if encryption_response.id != 0x01 {
        bail!(
            "Expected encryption response packet, got {:?}",
            encryption_response.id
        );
    }
    let mut encryption_response = encryption_response.as_cursor();
	let shared_secret_length = encryption_response.read_var_int().await?;
	let mut shared_secret = vec![0u8; shared_secret_length as usize];
    let shared_secret = encryption_response.read_exact(&mut shared_secret).await?;
    let verify_token_length = encryption_response.read_var_int().await?;
	let mut verify_token = vec![0u8; verify_token_length as usize];
	let verify_token = encryption_response.read_exact(&mut verify_token).await?;

    // make auth request to mojang
    let response: MojangAuthResponse = reqwest::get(format!(
		"https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}&ip={}",
		username,
		"",
		"",
	))
    .await?
    .json()
    .await?;

    Ok(())
}

/// Handle play packets.
async fn handle_upstream_play(
    state: Arc<BridgeState>,
    client_rx: &mut OwnedReadHalf,
    server_tx: &mut OwnedWriteHalf,
) -> Result<()> {
    // buffers for reading data
    let mut data = [0u8; 1024];
    client_rx.read_exact(&mut data).await?;
    // lock cryptor and decrypt packet
    let raw = match {
        let mut client = state.client.write().await;
        client.cryptor.next_packet(&mut data).await?
    } {
        Some(raw) => raw,
        None => return Ok(()),
    };
    // write packet to server
    server_tx.write_all(&raw).await?;
    Ok(())
}
