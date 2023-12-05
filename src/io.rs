//! Defines IO extension traits for reading from streams.

use std::{fmt::Debug, io::Cursor};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use miniz_oxide::inflate::decompress_to_vec_zlib;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use uuid::Uuid;

static SEGMENT_BITS: u8 = 0x7F;
static CONTINUE_BIT: u8 = 0x80;

/// An uncompressed packet.
pub struct UncompressedPacket {
    pub id: i32,
    pub data: Vec<u8>,
}

impl UncompressedPacket {
    pub fn as_cursor(&self) -> Cursor<&Vec<u8>> {
        Cursor::new(&self.data)
    }

    pub fn as_cursor_mut(&mut self) -> Cursor<&mut Vec<u8>> {
        Cursor::new(&mut self.data)
    }

    pub async fn into_raw(self) -> Result<Vec<u8>> {
        let buf = vec![0u8; self.data.len() + var_int_length(self.id)];
        let mut cursor = Cursor::new(buf);
        cursor.write_var_int(self.id).await?;
        cursor.write_all(&self.data).await?;
        Ok(cursor.into_inner())
    }

    pub async fn decompress(self) -> Result<UncompressedPacket> {
        Ok(self)
    }
}

/// A compressed packet.
pub struct CompressedPacket {
    pub packet_length: i32,
    pub data_length: i32,
    pub compressed_data: Vec<u8>,
}

impl CompressedPacket {
    pub async fn decompress(self) -> Result<UncompressedPacket> {
        // if packet does not meet the threshold, simply spit it back out
        let mut buf = match self.data_length {
            0 => self.compressed_data,
            _ => decompress_to_vec_zlib(&self.compressed_data)
                .map_err(|_| anyhow!("failed to decompress packet"))?,
        };
        let mut cursor = Cursor::new(&buf);
        // read and remove packet id from data
        let id = cursor.read_var_int().await?;
        buf.drain(..var_int_length(id));
        Ok(UncompressedPacket { id, data: buf })
    }

    pub async fn into_raw(self) -> Result<Vec<u8>> {
        let buf = vec![
            0u8;
            self.compressed_data.len()
                + var_int_length(self.packet_length)
                + var_int_length(self.data_length)
        ];
        let mut cursor = Cursor::new(buf);
        cursor.write_var_int(self.packet_length).await?;
        cursor.write_var_int(self.data_length).await?;
        cursor.write_all(&self.compressed_data).await?;
        Ok(cursor.into_inner())
    }
}

/// A packet.
pub enum Packet {
    Uncompressed(UncompressedPacket),
    Compressed(CompressedPacket),
}

impl Packet {
	pub async fn decompress(self) -> Result<UncompressedPacket> {
		match self {
			Packet::Uncompressed(packet) => Ok(packet),
			Packet::Compressed(packet) => packet.decompress().await,
		}
	}

	pub async fn into_raw(self) -> Result<Vec<u8>> {
		match self {
			Packet::Uncompressed(packet) => packet.into_raw().await,
			Packet::Compressed(packet) => packet.into_raw().await,
		}
	}
}

fn var_int_length(mut x: i32) -> usize {
    let mut size = 1; // all var ints are at least 1 byte big
    loop {
        x >>= 7;
        if x != 0 {
            size += 1;
        }
        if x == 0 {
            break;
        }
    }

    size
}

#[async_trait]
pub trait ProtocolReadExt: AsyncRead + Debug {
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

    #[tracing::instrument(skip_all)]
    async fn read_uuid(&mut self) -> Result<Uuid>
    where
        Self: Unpin,
    {
        let mut buf = [0u8; 16];
        self.read_exact(&mut buf).await?;
        let uuid = Uuid::from_bytes(buf);
        Ok(uuid)
    }

    #[tracing::instrument(skip_all)]
    async fn read_uncompressed_packet(&mut self) -> Result<UncompressedPacket>
    where
        Self: Unpin,
    {
        let length = self.read_var_int().await? as usize;
        if length == 0 {
            bail!("Attempted to read empty packet")
        }

        // read packet id and compute data length
        let id = self.read_var_int().await?;
        let data_length = length - var_int_length(id);

        // read data
        let mut data = vec![0u8; data_length];
        self.read_exact(&mut data).await?;

        Ok(UncompressedPacket { id, data })
    }

    /// Read a compressed packet from the stream. This does not decompress the packet.
    async fn read_compressed_packet(&mut self) -> Result<CompressedPacket>
    where
        Self: Unpin,
    {
        let packet_length = self.read_var_int().await?;
        let data_length = self.read_var_int().await?;

        // read compressed data
        let mut compressed_data = vec![0u8; data_length as usize];
        self.read_exact(&mut compressed_data).await?;

        Ok(CompressedPacket {
            packet_length,
            data_length,
            compressed_data,
        })
    }
}

#[async_trait]
pub trait ProcotolWriteExt: AsyncWrite {
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

    async fn write_var_long(&mut self, value: i64) -> Result<()>
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

    async fn write_uncompressed_packet(&mut self, packet: &UncompressedPacket) -> Result<()>
    where
        Self: Unpin,
    {
        let id_length = var_int_length(packet.id);
        self.write_var_int((packet.data.len() + id_length) as i32)
            .await?;
        self.write_var_int(packet.id).await?;
        Ok(())
    }

    async fn write_compressed_packet(&mut self, packet: &CompressedPacket) -> Result<()>
    where
        Self: Unpin,
    {
        self.write_var_int(packet.packet_length).await?;
        self.write_var_int(packet.data_length).await?;
        self.write_all(&packet.compressed_data).await?;
        Ok(())
    }

    async fn write_packet(&mut self, packet: &Packet) -> Result<()>
    where
        Self: Unpin,
    {
        match packet {
            Packet::Uncompressed(packet) => self.write_uncompressed_packet(packet).await?,
            Packet::Compressed(packet) => self.write_compressed_packet(packet).await?,
        }
        Ok(())
    }
}

// blanket implementations
impl<T: AsyncRead + Debug> ProtocolReadExt for T {}
impl<T: AsyncWrite> ProcotolWriteExt for T {}
