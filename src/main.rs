use std::net::SocketAddr;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use rsa::{RsaPrivateKey, RsaPublicKey};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

static SEGMENT_BITS: u8 = 0x7F;
static CONTINUE_BIT: u8 = 0x80;

#[async_trait]
trait ProtocolReadExt: AsyncRead {
    async fn read_var_int(&mut self) -> Result<i32>
    where
        Self: Unpin,
    {
        let mut num_read = 0;
        let mut result = 0;

        loop {
            let read = self.read_u8().await?;
            let value = i32::from(read & 0b0111_1111);
            result |= value.overflowing_shl(7 * num_read).0;

            num_read += 1;

            if num_read > 5 {
                bail!("VarInt too long (max length: 5)");
            }
            if read & 0b1000_0000 == 0 {
                break;
            }
        }

        Ok(result)
    }

    async fn read_var_long(&mut self) -> Result<u64>
    where
        Self: Unpin,
    {
        let mut value = 0;
        let mut position = 0;
        let mut current_byte: u8;

        loop {
            current_byte = self.read_u8().await.context("failed to read byte")?;
            value |= ((current_byte & SEGMENT_BITS) as u64) << position;

            if (current_byte & CONTINUE_BIT) == 0 {
                break Ok(value);
            }

            position += 7;

            if position >= 64 {
                bail!("VarInt exceeded maximum length");
            }
        }
    }

    async fn read_string(&mut self) -> Result<String>
    where
        Self: Unpin,
    {
        let len = self.read_var_int().await? as usize;
        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf)
            .await
            .context("failed to read string bytes")?;
        String::from_utf8(buf).context("failed to decode string bytes")
    }
}

#[async_trait]
trait ProcotolWriteExt: AsyncWrite {
    async fn write_var_int(&mut self, value: i32) -> Result<()>
    where
        Self: Unpin,
    {
        let mut x = value as u32;
        loop {
            let mut temp = (x & 0b0111_1111) as u8;
            x >>= 7;
            if x != 0 {
                temp |= 0b1000_0000;
            }
            self.write_all(&[temp]).await?;
            if x == 0 {
                break;
            }
        }
        Ok(())
    }

    async fn write_var_long(&mut self, mut value: i64) -> Result<()>
    where
        Self: Unpin,
    {
        let mut x = value as u64;
        loop {
            let mut temp = (x & 0b0111_1111) as u8;
            x >>= 7;
            if x != 0 {
                temp |= 0b1000_0000;
            }

            self.write_u8(temp).await?;

            if x == 0 {
                break;
            }
        }

        Ok(())
    }

    async fn write_string(&mut self, value: String) -> Result<()>
    where
        Self: Unpin,
    {
        self.write_var_int(value.len() as i32)
            .await
            .context("failed to write string length")?;
        let buf = value.as_bytes();
        self.write_all(buf).await.context("failed to write string")
    }
}

// blanket implementations
impl<T: AsyncRead> ProtocolReadExt for T {}
impl<T: AsyncWrite> ProcotolWriteExt for T {}

pub struct ClientEncryption {
    pub public_key: RsaPublicKey,
    pub private_key: RsaPrivateKey,
}

/// A bridge between a client and a server.
pub struct Bridge {
    pub client: TcpStream,
    pub client_addr: SocketAddr,
    pub client_encryption: ClientEncryption,
    pub server: TcpStream,
    pub server_addr: SocketAddr,
}

async fn handle_connection(stream: &mut TcpStream, addr: SocketAddr) -> Result<()> {
    // handshaking state
    let length = stream
        .read_var_int()
        .await
        .context("failed to read packet length")?;
    let packet_id = stream
        .read_var_int()
        .await
        .context("failed to read packet id")?;
    // expect packet id
    if packet_id != 0x00 {
        bail!("invalid packet id: {}", packet_id);
    }
    // read handshake packet
    let protocol_version = stream
        .read_var_int()
        .await
        .context("failed to read protocol version")?;
    let server_address = stream
        .read_string()
        .await
        .context("failed to read server address")?;
    let server_port = stream
        .read_u16()
        .await
        .context("failed to read server port")?;
    let next_state = stream
        .read_var_int()
        .await
        .context("failed to read next state")?;

    match next_state {
        1 => handle_status(stream).await,
        2 => handle_login(stream).await,
        _ => bail!("unexpected state: {:?}", next_state),
    }
}

async fn handle_status(stream: &mut TcpStream) -> Result<()> {
    println!("handling server status");
    loop {
        let length = stream
            .read_var_int()
            .await
            .context("failed to read packet length")?;
        let packet_id = stream
            .read_var_int()
            .await
            .context("failed to read packet id")?;

        match packet_id {
            0x00 => {
                println!("writing status packet");
                // write packet id and response to output
                let mut buf: Vec<u8> = Vec::new();
                buf.write_var_int(0x00).await.unwrap();
                buf.write_string(
                    r#"{
    "version": {
        "name": "1.19",
        "protocol": 759
    },
    "players": {
        "max": 100,
        "online": 5,
        "sample": [
            {
                "name": "thinkofdeath",
                "id": "4566e69f-c907-48ee-8d71-d7ba5aa00d20"
            }
        ]
    },
    "description": {
        "text": "Hello world"
    },
    "enforcesSecureChat": false
}"#
                    .to_string(),
                )
                .await
                .unwrap();
                // write packet length and data
                let len = buf.len();
                stream.write_var_int(len as i32).await.unwrap();
                stream.write_all_buf(&mut buf.as_slice()).await.unwrap();
                println!("done");
            }
            0x01 => {
                println!("writing ping packet");
                let nonce = stream.read_u64().await.unwrap();
                // write packet id and nonce
                let mut buf: Vec<u8> = Vec::new();
                buf.write_var_int(0x01).await.unwrap();
                buf.write_u64(nonce).await.unwrap();
                // write packet length and data
                let len = buf.len();
                stream.write_var_int(len as i32).await.unwrap();
                stream.write_all_buf(&mut buf.as_slice()).await.unwrap();
            }
            _ => bail!("invalid packet id: {}", packet_id),
        }
    }
}

async fn handle_login(stream: &mut TcpStream) -> Result<()> {
    println!("handling login");
    Ok(())
}

#[tokio::main]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:25565"
        .parse()
        .context("failed to parse socket address")
        .unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();

    loop {
        match listener.accept().await {
            Ok((mut stream, socket)) => {
                tokio::task::spawn(async move {
                    let res = handle_connection(&mut stream, addr).await;
                    if let Err(err) = res {
                        println!("{:?}", err);
                        stream
                            .shutdown()
                            .await
                            .expect("failed to shutdown connection");
                    }
                });
            }
            Err(err) => {
                println!("{:?}", err);
            }
        }
    }
}
