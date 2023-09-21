//! This module contains the various witness types.

use crate::{State, StateWitness, StateWitnessHasher};
use alloy_primitives::{keccak256, B256};

/// The size of an encoded [StateWitness] in bytes.
pub(crate) const STATE_WITNESS_SIZE: usize = 226;

impl StateWitnessHasher for StateWitness {
    fn state_hash(&self) -> B256 {
        let mut hash = keccak256(self);
        let offset = 32 * 2 + 4 * 6;
        let exit_code = self[offset];
        let exited = self[offset + 1] == 1;
        hash[0] = State::vm_status(exited, exit_code) as u8;
        hash
    }
}
