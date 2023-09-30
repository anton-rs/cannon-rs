//! This module contains the types for the preimage-oracle crate.

use crate::{Hint, Key};
use anyhow::Result;

/// A [PreimageGetter] is a function that can be used to fetch pre-images.
pub type PreimageGetter = Box<dyn Fn([u8; 32]) -> Result<Vec<u8>>>;

/// A [HintHandler] is a function that can be used to handle hints from a [crate::HintWriter].
pub type HintHandler = Box<dyn Fn(&[u8]) -> Result<()>>;

/// A [Keccak256Key] wraps a keccak256 hash to use it as a typed pre-image key.
pub type Keccak256Key = [u8; 32];

/// A [LocalIndexKey] is a key local to the program, indexing a special program input.
pub type LocalIndexKey = u64;

/// The [KeyType] enum represents the different types of keys that can be used to index
/// pre-images.
#[repr(u8)]
pub enum KeyType {
    /// The zero key type is illegal to use.
    _Illegal = 0,
    /// The local key type is used to index a local variable, specific to the program instance.
    Local = 1,
    /// The global key type is used to index a global keccak256 preimage.
    GlobalKeccak = 2,
}

/// The [PreimageFds] enum represents the file descriptors used for hinting and pre-image
/// communication.
#[repr(u8)]
pub enum PreimageFds {
    HintClientRead = 3,
    HintClientWrite = 4,
    PreimageClientRead = 5,
    PreimageClientWrite = 6,
}

impl From<u8> for KeyType {
    fn from(n: u8) -> Self {
        match n {
            1 => KeyType::Local,
            2 => KeyType::GlobalKeccak,
            _ => KeyType::_Illegal,
        }
    }
}

impl Key for LocalIndexKey {
    fn preimage_key(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[0] = KeyType::Local as u8;
        out[24..].copy_from_slice(&self.to_be_bytes());
        out
    }
}

impl Key for Keccak256Key {
    fn preimage_key(mut self) -> [u8; 32] {
        self[0] = KeyType::GlobalKeccak as u8;
        self
    }
}

impl Hint for &[u8] {
    fn hint(&self) -> &[u8] {
        self
    }
}
