//! The bridge proxies data from a single client TcpStream to a single server TcpStream.

use std::net::SocketAddr;

use anyhow::Result;
use tokio::{
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    try_join,
};
use tracing::debug;

use crate::{
    cryptor::Cryptor,
    io::{ProcotolWriteExt, ProtocolReadExt},
};

/// Consume the provided streams and bridge data between them.
#[tracing::instrument(skip_all, name = "bridge", fields(server_addr))]
pub async fn create(
    client_addr: SocketAddr,
    client_stream: TcpStream,
    server_addr: SocketAddr,
    server_stream: TcpStream,
) -> Result<()> {
    let (client_rx, client_tx) = client_stream.into_split();
    let (server_rx, server_tx) = server_stream.into_split();

	debug!("Bridge initialized");

    let client_to_server =
        tokio::task::spawn(async move { handle_client_to_server(client_rx, server_tx).await });

    let server_to_client =
        tokio::task::spawn(async move { handle_server_to_client(server_rx, client_tx).await });

    try_join!(client_to_server, server_to_client)
        .map(|_| ())
        .map_err(|e| e.into())
}

async fn handle_client_to_server(
    mut client_rx: OwnedReadHalf,
    mut server_tx: OwnedWriteHalf,
) -> Result<()> {
    // setup client state
    let encrypted = false;
    let compressed = false;
    let cryptor = Cryptor::Uninitialized;

    // read and forward packets to server
    loop {
        let packet = client_rx.read_packet().await?;
        server_tx
            .write_packet(packet.id as i32, &packet.data)
            .await?;
    }
}

async fn handle_server_to_client(
    mut server_rx: OwnedReadHalf,
    mut client_tx: OwnedWriteHalf,
) -> Result<()> {
    // setup client state
    let encrypted = false;
    let compressed = false;
    let cryptor = Cryptor::Uninitialized;

    // read and forward packets to server
    loop {
        let packet = server_rx.read_packet().await?;
        client_tx
            .write_packet(packet.id as i32, &packet.data)
            .await?;
    }
}
