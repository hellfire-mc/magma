//! Defines IO extension traits for reading from streams.

use std::io::Cursor;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use miniz_oxide::{deflate::compress_to_vec_zlib, inflate::decompress_to_vec_zlib};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

static SEGMENT_BITS: u8 = 0x7F;
static CONTINUE_BIT: u8 = 0x80;

fn var_int_length(mut x: i32) -> usize {
    let mut size = 0;
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
pub trait ProtocolReadExt: AsyncRead {
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

    async fn read_packet(&mut self) -> Result<Cursor<Vec<u8>>>
    where
        Self: Unpin,
    {
        let len = self.read_var_int().await? as usize;
        let packet_id = self.read_var_int().await?;
        let buf = vec![0u8; len - var_int_length(packet_id)];
        Ok(Cursor::new(buf))
    }

    async fn read_packet_compressed(&mut self) -> Result<Cursor<Vec<u8>>>
    where
        Self: Unpin,
    {
        let len = self.read_var_int().await?;
        let data_len = self.read_var_int().await?;
        // read packet data
        let mut buf = vec![0u8; len as usize];
        self.read_exact(&mut buf).await?;
        // packet is uncompressed
        if data_len == 0 {
            return Ok(Cursor::new(buf));
        }
        // decompress data
        let buf =
            decompress_to_vec_zlib(&buf).map_err(|_| anyhow!("failed to decompress packet"))?;
        Ok(Cursor::new(buf))
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

    async fn write_packet(&mut self, value: &Vec<u8>) -> Result<()>
    where
        Self: Unpin,
    {
        self.write_var_int(value.len() as i32).await?;
        self.write_all(value).await?;
        Ok(())
    }

    async fn write_packet_compressed(&mut self, value: &Vec<u8>) -> Result<()>
    where
        Self: Unpin,
    {
        let buf = compress_to_vec_zlib(value, 10);
        self.write_var_int(buf.len() as i32).await?;
        self.write_var_int(value.len() as i32).await?;
        self.write_all(&buf).await?;
        Ok(())
    }
}

// blanket implementations
impl<T: AsyncRead> ProtocolReadExt for T {}
impl<T: AsyncWrite> ProcotolWriteExt for T {}
