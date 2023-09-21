//! This module contains the various traits used in this crate.

use alloy_primitives::B256;

/// A [StateWitnessHasher] is a trait for the [crate::StateWitness] type to
/// compute its witness hash.
pub trait StateWitnessHasher {
    /// Compute the [crate::StateWitness] hash.
    fn state_hash(&self) -> B256;
}
