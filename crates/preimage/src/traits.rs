//! This module contains the traits for the preimage-oracle crate.

use alloy_primitives::B256;
use anyhow::Result;

/// The [Key] trait describes the behavior of a pre-image key that may be wrapped
/// into a 32-byte type-prefixed key.
pub trait Key {
    /// Changes the [Key] commitment into a 32-byte type-prefixed preimage key.
    fn preimage_key(self) -> B256;
}

/// The [Oracle] trait describes the behavior of a read-only pre-image oracle.
pub trait Oracle {
    /// Get the full pre-image of a given pre-image key.
    fn get(&mut self, key: impl Key) -> Result<Vec<u8>>;
}

// [Hint] is an trait to enable any program type to function as a hint,
// When passed to the Hinter interface, returning a string representation
// of what data the host should prepare pre-images for.
pub trait Hint {
    /// Returns a string representation of the data the host should prepare
    /// pre-images for.
    fn hint() -> String;
}

// [Hinter] is an trait describing behavior for writing hints to the host.
// This may be implemented as a no-op or logging hinter if the program is executing
// in a read-only environment where the host is expected to have all pre-images ready.
pub trait Hinter {
    /// Returns a string representation of the data the host should prepare
    /// pre-images for.
    fn hint(&self) -> String;
}
