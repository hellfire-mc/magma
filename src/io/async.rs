//! Defines extension traits for asynchronously reading and writing Minecraft
//! packets to a type implementing [AsyncRead].

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

use std::fmt::Debug;

use super::{
    var_int_length, CompressedPacket, Packet, UncompressedPacket, VARINT_CONTINUE_BIT,
    VARINT_SEGMENT_BITS,
};

/// Extension trait for reading Minecraft packets from a stream.
#[async_trait]
pub trait ProtocolAsyncReadExt: AsyncRead {
    /// Read a var int from the stream.
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

    /// Read a var long from the stream.
    async fn read_var_long(&mut self) -> Result<u64>
    where
        Self: Unpin,
    {
        let mut value = 0;
        let mut position = 0;
        let mut current_byte: u8;

        loop {
            current_byte = self.read_u8().await.context("failed to read byte")?;
            value |= ((current_byte & VARINT_SEGMENT_BITS) as u64) << position;

            if (current_byte & VARINT_CONTINUE_BIT) == 0 {
                break Ok(value);
            }

            position += 7;

            if position >= 64 {
                bail!("VarInt exceeded maximum length");
            }
        }
    }

    /// Read a string from the stream.
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

    /// Read a [Uuid] from the stream.
    async fn read_uuid(&mut self) -> Result<Uuid>
    where
        Self: Unpin,
    {
        let mut buf = [0u8; 16];
        self.read_exact(&mut buf).await?;
        let uuid = Uuid::from_bytes(buf);
        Ok(uuid)
    }

    /// Read an [UncompressedPacket] from the stream.
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

/// Extension trait for writing Minecraft packets to a stream.
#[async_trait]
pub trait ProcotolAsyncWriteExt: AsyncWrite {
    /// Write a var int to the stream.
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

    /// Write a var long to the stream.
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

    /// Write a string to the stream.
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

    /// Write an [UncompressedPacket] to the stream.
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

    /// Write a [CompressedPacket] to the stream.
    async fn write_compressed_packet(&mut self, packet: &CompressedPacket) -> Result<()>
    where
        Self: Unpin,
    {
        self.write_var_int(packet.packet_length).await?;
        self.write_var_int(packet.data_length).await?;
        self.write_all(&packet.compressed_data).await?;
        Ok(())
    }

    /// Write a [Packet] to the stream.
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
impl<T: AsyncRead + Debug> ProtocolAsyncReadExt for T {}
impl<T: AsyncWrite> ProcotolAsyncWriteExt for T {}
