//! This module contains the traits for the preimage-oracle crate.

use std::{
    fs::File,
    io::{Read, Write},
};

use anyhow::Result;

/// The [Key] trait describes the behavior of a pre-image key that may be wrapped
/// into a 32-byte type-prefixed key.
pub trait Key {
    /// Changes the [Key] commitment into a 32-byte type-prefixed preimage key.
    fn preimage_key(self) -> [u8; 32];
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
    fn hint(&self) -> &[u8];
}

// [Hinter] is an trait describing behavior for writing hints to the host.
// This may be implemented as a no-op or logging hinter if the program is executing
// in a read-only environment where the host is expected to have all pre-images ready.
pub trait Hinter {
    /// Sends a hint to the host.
    ///
    /// ### Takes
    /// - `hint` - The hint to send to the host.
    ///
    /// ### Returns
    /// - A [Result] indicating whether or not the hint was successfully sent.
    fn hint<T: Hint>(&self, hint: T) -> Result<()>;
}

/// The [FileChannel] trait represents a dual channel that can be used to read
/// and write information to file descriptors.
pub trait FileChannel: Read + Write {
    /// Returns the reader file descriptor.
    fn reader(&mut self) -> &mut File;

    /// Returns the writer file descriptor.
    fn writer(&mut self) -> &mut File;

    /// Closes the file descriptors.
    fn close(self) -> Result<()>;
}
