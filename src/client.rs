//! Contains a basic Minecraft proxy client for connecting to remote servers.

use std::{io::Cursor, net::SocketAddr};

use anyhow::{bail, Context, Result};
use rand::RngCore;
use rsa::{pkcs8::DecodePublicKey, PaddingScheme, PublicKey, RsaPublicKey};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use uuid::Uuid;

use crate::{config::SelectionAlgorithm, cryptor::Cryptor, ProcotolWriteExt, ProtocolReadExt};

pub struct Client {
    pub addr: SocketAddr,
    pub player_name: String,
    pub player_uuid: Uuid,
    pub stream: TcpStream,
    pub encrypted: bool,
}

impl Client {
    async fn connect(addr: SocketAddr, player_name: String, player_uuid: Uuid) -> Result<Self> {
        let mut stream = TcpStream::connect(&addr)
            .await
            .context("failed to connect to address")?;

        Ok(Self {
            addr,
            player_name,
            player_uuid,
            stream,
            encrypted: false,
        })
    }

    /// Perform the client handshake.
    async fn handshake(&mut self) -> Result<()> {
        // write handshake packet
        let mut buf = Vec::new();
        buf.write_var_int(0x00).await?;
        buf.write_var_int(761).await?;
        buf.write_string(self.addr.ip().to_string()).await?;
        buf.write_u16(self.addr.port()).await?;
        buf.write_var_int(2).await?;
        self.stream.write_packet(&buf).await?;

        // write login start packet
        let mut buf = Vec::new();
        buf.write_var_int(0x00).await?;
        buf.write_string(self.player_name.clone()).await?;
        buf.write_u8(0x01).await?;
        buf.write_u128(self.player_uuid.as_u128()).await?;
        self.stream.write_packet(&buf).await?;

        let mut cryptor: Option<Cryptor> = None;
        loop {
            let packet_id;
            // handle encryption
            if self.encrypted {
                let c = cryptor.as_mut().unwrap();
                let mut block = [0u8; 128];
                self.stream.read_exact(&mut block).await?;
                let packet = c.next_packet(&mut block).await?;

                match packet {
                    None => {
                        continue;
                    }
                    Some(data) => {
                        let mut data = Cursor::new(data);
                        data.read_var_int().await?;
                        packet_id = data.read_var_int().await?;
                    }
                }
            } else {
                self.stream.read_var_int().await?;
                packet_id = self.stream.read_var_int().await?;
            }

            match packet_id {
                0x00 => bail!("received disconnect"),
                0x01 => {
                    let _server_id = self.stream.read_string().await?;
                    assert_eq!(_server_id.len(), 0);
                    // read public key
                    let len = self.stream.read_var_int().await? as usize;
                    let mut buf = vec![0u8; len];
                    self.stream.read_exact(&mut buf).await?;
                    let public_key = RsaPublicKey::from_public_key_der(&buf)
                        .context("failed to decode public key")?;
                    // read verify token
                    let len = self.stream.read_var_int().await? as usize;
                    let mut verify_token = vec![0u8; len];
                    self.stream.read_buf(&mut verify_token).await?;
                    // generate secret
                    let mut secret = [0u8; 16];
                    rand::thread_rng().fill_bytes(&mut secret);
                    // encrypt secret and token
                    let encrypted_token = public_key.encrypt(
                        &mut rand::thread_rng(),
                        PaddingScheme::PKCS1v15Encrypt,
                        &verify_token,
                    )?;
                    let encrypted_secret = public_key.encrypt(
                        &mut rand::thread_rng(),
                        PaddingScheme::PKCS1v15Encrypt,
                        &verify_token,
                    )?;
                    // write encryption response packet
                    let mut buf = Vec::new();
                    buf.write_var_int(encrypted_secret.len() as i32).await?;
                    buf.write_all(&encrypted_secret).await?;
                    buf.write_var_int(encrypted_token.len() as i32).await?;
                    buf.write_all(&encrypted_secret).await?;
                    // enable encryption
                    self.encrypted = true;
                    cryptor = Some(Cryptor::new(&secret))
                }
                0x02 => {
                    // handshake successful
                    println!("handshake successful");
                    break;
                }
                _ => bail!("unknown packet id"),
            };
        }

        Ok(())
    }
}

pub struct ProxyTargetSelector {
    targets: Vec<SocketAddr>,
    selection_algorithm: SelectionAlgorithm,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use uuid::Uuid;

    use super::Client;

    #[tokio::test]
    async fn test() {
        let mut client = Client::connect(
            "127.0.0.1:25565".parse().unwrap(),
            "kaylendog".to_string(),
            Uuid::from_str("ec294b17377d4bc580eefa0c56de77b9").unwrap(),
        )
        .await
        .unwrap();
        client.handshake().await.unwrap();
    }
}
