//! The bridge proxies data from a single client TcpStream to a single server TcpStream.

use std::net::SocketAddr;

use anyhow::{bail, Context, Error, Result};
use tokio::{io::AsyncWriteExt, net::TcpStream};

use crate::{
    cryptor::Cryptor,
    io::{ProcotolWriteExt, ProtocolReadExt},
    protocol::ProtocolState,
};

/// A bridge between a client and a server.
pub struct Bridge {
    /// The client stream being proxied through the bridge.
    client: SocketAddr,
    /// The server handling the client's packets.
    proxy_server: BridgeServer,
    /// The server stream being proxied through the bridge.
    server: SocketAddr,
    /// The client handling the server's packets.
    proxy_client: BridgeClient,
}

impl Bridge {
    /// Create a new bridge from the given stream and remote address.
    async fn from_stream(
        stream: TcpStream,
        remote_addr: SocketAddr,
    ) -> Result<Option<Self>, Error> {
        // create the proxy server
        let mut proxy_server = BridgeServer::from_stream(stream).await;
        let handshake = proxy_server.handshake().await.context("handshake failed")?;
        // don't handle status
        if matches!(handshake.next_state, ProtocolState::Status) {
            return Ok(None);
        }
        // create proxy client and handshake
        let mut proxy_client = BridgeClient::connect(remote_addr).await?;
        proxy_client.handshake().await;
        // read login packet

        Ok(Some(Self {
            client: remote_addr,
            server: remote_addr,
            proxy_server,
            proxy_client,
        }))
    }
}

struct BridgeServer {
    state: ProtocolState,
    client_stream: TcpStream,
    cryptor: Cryptor,
}

impl BridgeServer {
    pub async fn from_stream(stream: TcpStream) -> Self {
        Self {
            state: ProtocolState::Handshaking,
            client_stream: stream,
            cryptor: Cryptor::new(),
        }
    }
    /// Accept a handshake package and transition into the next state.
    pub async fn handshake(&mut self) -> Result<HandshakeResult> {
        self.client_stream.read_var_int().await?; // packet length
                                                  // ensure correct packet id
        let packet_id = self.client_stream.read_var_int().await?; // packet id
        if packet_id != 0x00 {
            bail!("invalid packet id: {}", packet_id);
        }
        // read handshake packet
        let protocol_version = self.client_stream.read_var_int().await?; // protocol version
        let server_address = self.client_stream.read_string().await?; // server address
        self.client_stream.read_var_int().await?; // server port
        let next_state = match self.client_stream.read_var_int().await? {
            1 => ProtocolState::Status,
            2 => ProtocolState::Login,
            _ => bail!("invalid next state"),
        };
        Ok(HandshakeResult {
            next_state,
            server_address,
            protocol_version: protocol_version
                .try_into()
                .context("failed to cast protocol version")?,
        })
    }

    /// Accept a login packet.
    pub async fn login(&mut self) -> Result<()> {
        let packet = self.client_stream.read_packet();
        Ok(())
    }
}

pub struct HandshakeResult {
    pub next_state: ProtocolState,
    pub server_address: String,
    pub protocol_version: u16,
}

struct BridgeClient {
    remote_addr: SocketAddr,
    server_stream: TcpStream,
    cryptor: Cryptor,
}

impl BridgeClient {
    pub async fn connect(remote_addr: SocketAddr) -> Result<Self> {
        let server_stream = TcpStream::connect(remote_addr)
            .await
            .context("failed to connect to server")?;
        Ok(Self {
            remote_addr,
            server_stream,
            cryptor: Cryptor::new(),
        })
    }

    /// Write and send the handshake packet.
    pub async fn handshake(&mut self) -> Result<()> {
        // write handshake packet
        let mut buf = Vec::new();
        buf.write_var_int(0x00).await?;
        buf.write_var_int(761).await?;
        buf.write_string(self.remote_addr.ip().to_string()).await?;
        buf.write_u16(self.remote_addr.port()).await?;
        buf.write_var_int(2).await?;
        self.server_stream.write_packet(&buf).await?;

        Ok(())
    }
}
