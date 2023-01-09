use std::io::{Cursor, Read};

use aes::{
    cipher::{AsyncStreamCipher, BlockDecryptMut, KeyIvInit},
    Aes128,
};
use anyhow::Result;

use crate::ProtocolReadExt;

type Decryptor = cfb8::Decryptor<Aes128>;
type Encryptor = cfb8::Encryptor<Aes128>;

pub struct Cryptor {
    pub encryptor: Encryptor,
    pub decryptor: Decryptor,
    pub buffer: Vec<u8>,
}

impl Cryptor {
    /// Create a new cryptor instance.
    pub fn new(key: &[u8]) -> Self {
        Self {
            buffer: Vec::with_capacity(512),
            decryptor: Decryptor::new(key.into(), key.into()),
            encryptor: Encryptor::new(key.into(), key.into()),
        }
    }

    /// Read the next packet from the stream.
    pub async fn next_packet(&mut self, data: &mut [u8]) -> Result<Option<Vec<u8>>> {
        // decrypt data
        self.decryptor.decrypt_block_mut(data.into());
        self.buffer.extend_from_slice(data);
        // create cursor and read packet length
        let mut cursor = Cursor::new(&self.buffer);
        let packet_length = cursor.read_var_int().await? as usize;
        // attempt to fetch data - could make this zero copy
        let mut buf = vec![0u8; packet_length];
        let bytes_read = cursor.read(&mut buf)?;
        // ensure we have a full packet
        if bytes_read < packet_length {
            return Ok(None);
        }
        // update internal buffer
        self.buffer.drain(0..packet_length);
        Ok(Some(buf))
    }
}
