//! Defines IO extension traits for reading Minecraft packets from streams.
//!
//! Packets in Minecraft are sent either uncompressed or compressed, using zlib compression.
//! Ideally, Magma should do as little processing as possible on packets, and should only
//! decompress packets when it needs to read the data inside.
//!
//! Refer to the [wiki.vg](https://wiki.vg/Protocol#Packet_format) for more information on
//! Minecraft packet formats.

use std::io::{Cursor, Write};

use anyhow::{anyhow, Result};
use miniz_oxide::inflate::decompress_to_vec_zlib;

mod r#async;
mod sync;

pub use r#async::{ProcotolAsyncWriteExt, ProtocolAsyncReadExt};
pub use sync::{ProtocolReadExt, ProtocolWriteExt};

/// An uncompressed packet.
pub struct UncompressedPacket {
    /// The packet id.
    pub id: i32,
    /// The packet data.
    pub data: Vec<u8>,
}

impl UncompressedPacket {
    /// Returns a cursor over the packet data.
    pub fn as_cursor(&self) -> Cursor<&Vec<u8>> {
        Cursor::new(&self.data)
    }

    /// Returns a mutable cursor over the packet data.
    pub fn as_cursor_mut(&mut self) -> Cursor<&mut Vec<u8>> {
        Cursor::new(&mut self.data)
    }

    /// Consumes the packet and returns its raw bytes.
    pub fn into_raw(self) -> Result<Vec<u8>> {
        let buf = vec![0u8; self.data.len() + var_int_length(self.id)];
        let mut cursor = Cursor::new(buf);
        ProtocolWriteExt::write_var_int(&mut cursor, self.id)?;
        cursor.write_all(&self.data)?;
        Ok(cursor.into_inner())
    }
}

/// A compressed packet.
pub struct CompressedPacket {
    /// The length of the packet.
    pub packet_length: i32,
    /// The length of the uncompressed data.
    pub data_length: i32,
    /// The compressed data.
    pub compressed_data: Vec<u8>,
}

impl CompressedPacket {
    /// Decompresses the packet.
    ///
    /// This is a no-op if the packet does not meet the compression threshold -
    /// see [Packet Format](https://wiki.vg/Protocol#Packet_format) for more information.
    ///
    /// **Decompression is expensive!** Avoid calling this method unless you need to **really**
    /// read the data inside the packet.
    pub fn decompress(self) -> Result<UncompressedPacket> {
        // if packet does not meet the threshold, simply spit it back out
        let mut data = match self.data_length {
            0 => self.compressed_data,
            _ => decompress_to_vec_zlib(&self.compressed_data)
                .map_err(|_| anyhow!("failed to decompress packet"))?,
        };
        let mut cursor = Cursor::new(&data);
        // read and remove packet id from data
        let id = ProtocolReadExt::read_var_int(&mut cursor)?;
        data.drain(..var_int_length(id));
        Ok(UncompressedPacket { id, data })
    }

    /// Consumes the packet and returns its raw bytes.
    pub fn into_raw(self) -> Result<Vec<u8>> {
        let buf = vec![
            0u8;
            self.compressed_data.len()
                + var_int_length(self.packet_length)
                + var_int_length(self.data_length)
        ];
        let mut cursor = Cursor::new(buf);
        ProtocolWriteExt::write_var_int(&mut cursor, self.packet_length)?;
        ProtocolWriteExt::write_var_int(&mut cursor, self.data_length)?;
        cursor.write_all(&self.compressed_data)?;
        Ok(cursor.into_inner())
    }
}

/// A packet, which may be compressed or uncompressed.
pub enum Packet {
    /// An uncompressed packet.
    Uncompressed(UncompressedPacket),
    /// A compressed packet.
    Compressed(CompressedPacket),
}

impl Packet {
    /// Decompresses the packet if it is compressed.
    pub fn decompress(self) -> Result<UncompressedPacket> {
        match self {
            Packet::Uncompressed(packet) => Ok(packet),
            Packet::Compressed(packet) => packet.decompress(),
        }
    }

    /// Consumes the packet and returns its raw bytes.
    pub fn into_raw(self) -> Result<Vec<u8>> {
        match self {
            Packet::Uncompressed(packet) => packet.into_raw(),
            Packet::Compressed(packet) => packet.into_raw()
        }
    }
}

/// Calculates the length of a var int.
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

/// Used to extract the value from a segment.
const VARINT_SEGMENT_BITS: u8 = 0x7F;
/// Used to indicate whether there are more bytes to read.
const VARINT_CONTINUE_BIT: u8 = 0x80;
