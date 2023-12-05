//! Handles encryption and decryption of packets between the client and proxy.

use std::io::{Cursor, Read};

use aes::{
    cipher::{BlockDecryptMut, KeyIvInit},
    Aes128,
};
use anyhow::Result;

use crate::io::ProtocolAsyncReadExt;

type Decryptor = cfb8::Decryptor<Aes128>;
type Encryptor = cfb8::Encryptor<Aes128>;

#[derive(Default)]
pub enum Cryptor {
    #[default]
    Uninitialized,
    Initialized {
        encryptor: Box<Encryptor>,
        decryptor: Box<Decryptor>,
        inbuffer: Vec<u8>,
        outbuffer: Vec<u8>,
    },
}

impl Cryptor {
    /// Create a new cryptor instance.
    pub fn new(key: &[u8]) -> Self {
        Self::Initialized {
            inbuffer: Vec::with_capacity(512),
            outbuffer: Vec::with_capacity(512),
            decryptor: Box::new(Decryptor::new(key.into(), key.into())),
            encryptor: Box::new(Encryptor::new(key.into(), key.into())),
        }
    }

    /// Read the next packet from the stream.
    pub async fn next_packet(&mut self, data: &mut [u8]) -> Result<Option<Vec<u8>>> {
        let (decryptor, buffer) = match self {
            Cryptor::Initialized {
                encryptor: _,
                decryptor,
                inbuffer,
                outbuffer: _,
            } => (decryptor, inbuffer),
            _ => panic!(),
        };
        // decrypt data
        decryptor.decrypt_block_mut(data.into());
        buffer.extend_from_slice(data);
        // create cursor and read packet length
        let mut cursor = Cursor::new(&buffer);
        let packet_length = cursor.read_var_int().await? as usize;
        // attempt to fetch data - could make this zero copy
        let mut buf = vec![0u8; packet_length];
        let bytes_read = cursor.read(&mut buf)?;
        // ensure we have a full packet
        if bytes_read < packet_length {
            return Ok(None);
        }
        // update internal buffer
        buffer.drain(0..packet_length);
        Ok(Some(buf))
    }

    /// Encrypt the given data.
    pub fn encrypt_packet(&mut self, data: &[u8]) -> Vec<u8> {
        todo!()
    }
}
