//! The bridge proxies data from a single client TcpStream to a single server TcpStream.

use std::net::SocketAddr;

use anyhow::{bail, Context, Error, Result};
use mc_chat::{ChatComponent, ComponentStyle};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::{debug, info};

use crate::{
    cryptor::Cryptor,
    io::{ProcotolWriteExt, ProtocolReadExt},
    protocol::{ProtocolState, StatusResponse, StatusResponsePlayers, StatusResponseVersion},
    proxy::{
        ProxyServerConfig, SelectionAlgorithm,
    },
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
    #[tracing::instrument(skip_all, name="bridge", fields(client=%remote_addr))]
    pub async fn from_stream(
        config: &ProxyServerConfig,
        stream: TcpStream,
        remote_addr: SocketAddr,
    ) -> Result<Option<Self>, Error> {
        info!("New connection to proxy");

        // create the proxy server
        let mut proxy_server = BridgeServer::from_stream(stream).await;
        let handshake = proxy_server.handshake().await.context("Handshake failed")?;
        // find target routes
        let route = config
            .routes
            .iter()
            .find(|r| r.from == handshake.server_address);

        if route.is_none() {
            info!(
                "Requested server {} was not found",
                handshake.server_address
            );

            // handle status
            if matches!(handshake.next_state, ProtocolState::Status) {
                debug!("Client requested server status");

                proxy_server
                    .status(ChatComponent::from_text(
                        "Unrecognised server",
                        ComponentStyle::v1_16(),
                    ))
                    .await?;

                info!("Disconnected from proxy");

                return Ok(None);
            }
            // immediately disconnect
            proxy_server
                .login_disconnect(ChatComponent::from_text(
                    format!("No route found for address '{}'", handshake.server_address),
                    ComponentStyle::v1_16(),
                ))
                .await?;

            info!("Disconnected from proxy");

            return Ok(None);
        }

		// attempt to connect to the remote server
        let route = route.unwrap();
        let proxy_client = BridgeClient::connect(*route.to.first().unwrap()).await;

        if let Err(_e) = proxy_client {
            // handle status
            if matches!(handshake.next_state, ProtocolState::Status) {
                debug!("Client requested server status");

                proxy_server
                    .status(ChatComponent::from_text(
                        "Failed to connect to remote server",
                        ComponentStyle::v1_16(),
                    ))
                    .await?;

                return Ok(None);
            }
            // immediately disconnect
            proxy_server
                .login_disconnect(ChatComponent::from_text(
                    "Failed to connect to remote server",
                    ComponentStyle::v1_16(),
                ))
                .await?;
            return Ok(None);
        }

		let _proxy_client = proxy_client.unwrap();

        // handle status
        if matches!(handshake.next_state, ProtocolState::Status) {
            debug!("Client requested server status");

            proxy_server
                .status(ChatComponent::from_text(
                    "Unrecognised server",
                    ComponentStyle::v1_16(),
                ))
                .await?;

            return Ok(None);
        }
        // create proxy client and handshake
        let mut proxy_client = BridgeClient::connect(remote_addr).await?;
        proxy_client.handshake().await?;
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
            cryptor: Cryptor::Uninitialized,
        }
    }

    /// Accept a handshake package and transition into the next state.
    #[tracing::instrument(skip_all)]
    pub async fn handshake(&mut self) -> Result<HandshakeResult> {
        let packet = self.client_stream.read_packet().await?;
        if packet.id != 0x00 {
            bail!("invalid packet id: {}", packet.id);
        }
        // read handshake packet data
        let mut data = packet.as_cursor();
        let protocol_version = data.read_var_int().await?;
        let server_address = data.read_string().await?;
        let server_port = data.read_u16().await?;
        let next_state = data.read_var_int().await?;
        debug!("Protocol version: {}", protocol_version);
        debug!("Server address: {}", server_address);
        debug!("Server port: {}", server_port);
        debug!("Next state: {}", next_state);
        // switch on next state
        let next_state = match next_state {
            1 => ProtocolState::Status,
            2 => ProtocolState::Login,
            s => bail!("Invalid next state: {}", s),
        };
        Ok(HandshakeResult {
            next_state,
            server_address,
            protocol_version: protocol_version
                .try_into()
                .context("Illegal protocol version")?,
        })
    }

    #[tracing::instrument(skip_all)]
    pub async fn status(&mut self, description: ChatComponent) -> Result<()> {
        let packet = self.client_stream.read_packet().await?;

        match packet.id {
            0x00 => {
                debug!("Sending server status...");
                let mut buf = Vec::new();
                let status = StatusResponse {
                    description,
                    enforces_secure_chat: false,
                    previews_chat: false,
                    favicon: "".to_string(),
                    players: StatusResponsePlayers {
                        max: 0,
                        online: 0,
                        sample: vec![],
                    },
                    version: StatusResponseVersion {
                        name: "Unknown".to_string(),
                        protocol: 0,
                    },
                };
                let status = serde_json::to_string(&status)?;
                buf.write_string(status).await?;
                self.client_stream.write_packet(0x00, &buf).await?;
            }
            0x01 => {
                debug!("Sending server ping...");
                // read incoming packet data
                let mut data = packet.as_cursor();
                let payload = data.read_u64().await?;
                // write outgoing packet data
                let mut buf = Vec::new();
                buf.write_u64(payload).await?;
                self.client_stream.write_packet(0x01, &buf).await?;
            }
            _ => bail!("invalid packet id: {}", packet.id),
        }

        Ok(())
    }

    /// Accept a login packet.
    pub async fn login(&mut self) -> Result<()> {
        let _packet = self.client_stream.read_packet();
        todo!("login implementation");
    }

    pub async fn login_disconnect(&mut self, reason: ChatComponent) -> Result<()> {
        // read login packet
        let _packet = self.client_stream.read_packet().await?;
        // write data
        let mut buf = vec![];
        let reason = serde_json::to_string(&reason)?;
        buf.write_string(reason).await?;
        self.client_stream.write_packet(0x00, &buf).await?;
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
            cryptor: Cryptor::Uninitialized,
        })
    }

    /// Write and send the handshake packet.
    pub async fn handshake(&mut self) -> Result<()> {
        // write handshake packet
        let mut buf = Vec::new();
        buf.write_var_int(761).await?;
        buf.write_string(self.remote_addr.ip().to_string()).await?;
        buf.write_u16(self.remote_addr.port()).await?;
        buf.write_var_int(2).await?;
        self.server_stream.write_packet(0x00, &buf).await?;

        Ok(())
    }
}
