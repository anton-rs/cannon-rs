//! This module contains the types for the preimage-oracle crate.

use crate::{traits::Hint, Key};
use alloy_primitives::B256;
use anyhow::Result;
use std::io::{BufReader, BufWriter, Read, Write};

#[repr(u8)]
pub enum KeyType {
    /// The zero key type is illegal to use.
    _Illegal = 0,
    /// The local key type is used to index a local variable, specific to the program instance.
    Local = 1,
    /// The global key type is used to index a global keccak256 preimage.
    GlobalKeccak = 2,
}

/// A [LocalIndexKey] is a key local to the program, indexing a special program input.
pub type LocalIndexKey = u64;

impl Key for LocalIndexKey {
    fn preimage_key(self) -> B256 {
        let mut out = B256::ZERO;
        out[0] = KeyType::Local as u8;
        out[24..].copy_from_slice(&self.to_be_bytes());
        out
    }
}

/// A [Keccak256Key] wraps a keccak256 hash to use it as a typed pre-image key.
pub type Keccak256Key = B256;

impl Key for Keccak256Key {
    fn preimage_key(mut self) -> B256 {
        self[0] = KeyType::GlobalKeccak as u8;
        self
    }
}

/// An [OracleFn] is a function that can be used to fetch pre-images.
pub type OracleFn = fn(key: dyn Key) -> Vec<u8>;

/// A [HinterFn] is a function that can be used to write a hint to the host.
pub type HinterFn = fn(hint: dyn Hint);

/// A [ReadWriterPair] is a wrapper around two types, implementing [Read] and [Write].
pub struct ReadWriterPair<R: Read, W: ?Sized + Write> {
    reader: BufReader<R>,
    writer: BufWriter<W>,
}

impl<R: Read, W: Write> ReadWriterPair<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
        }
    }
}

impl<R, W> Read for ReadWriterPair<R, W>
where
    R: Read,
    W: Write,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}

impl<R, W> Write for ReadWriterPair<R, W>
where
    R: Read,
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// A [PreimageGetter] is a function that can be used to fetch pre-images.
pub type PreimageGetter = Box<dyn Fn(B256) -> Result<Vec<u8>>>;
