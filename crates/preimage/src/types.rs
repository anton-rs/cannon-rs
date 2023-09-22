//! This module contains the types for the preimage-oracle crate.

use crate::Key;
use alloy_primitives::B256;
use anyhow::Result;

/// A [PreimageGetter] is a function that can be used to fetch pre-images.
pub type PreimageGetter = Box<dyn Fn(B256) -> Result<Vec<u8>>>;

/// A [Keccak256Key] wraps a keccak256 hash to use it as a typed pre-image key.
pub type Keccak256Key = B256;

/// A [LocalIndexKey] is a key local to the program, indexing a special program input.
pub type LocalIndexKey = u64;

#[repr(u8)]
pub enum KeyType {
    /// The zero key type is illegal to use.
    _Illegal = 0,
    /// The local key type is used to index a local variable, specific to the program instance.
    Local = 1,
    /// The global key type is used to index a global keccak256 preimage.
    GlobalKeccak = 2,
}

impl Key for LocalIndexKey {
    fn preimage_key(self) -> B256 {
        let mut out = B256::ZERO;
        out[0] = KeyType::Local as u8;
        out[24..].copy_from_slice(&self.to_be_bytes());
        out
    }
}

impl Key for Keccak256Key {
    fn preimage_key(mut self) -> B256 {
        self[0] = KeyType::GlobalKeccak as u8;
        self
    }
}
