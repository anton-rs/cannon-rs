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

/// A [StepWitness] is produced after each instruction step of the MIPS emulator. It contains
/// the encoded [StateWitness], the proof of memory access, and the preimage key, value, and
/// offset.
pub struct StepWitness {
    /// The encoded state witness
    pub state: StateWitness,
    /// The proof of memory access
    pub mem_proof: Vec<u8>,
    /// The preimage key
    pub preimage_key: B256,
    /// The preimage value
    pub preimage_value: Vec<u8>,
    /// The preimage offset
    pub preimage_offset: u32,
}

impl Default for StepWitness {
    fn default() -> Self {
        Self {
            state: [0u8; crate::witness::STATE_WITNESS_SIZE],
            mem_proof: Default::default(),
            preimage_key: Default::default(),
            preimage_value: Default::default(),
            preimage_offset: Default::default(),
        }
    }
}
