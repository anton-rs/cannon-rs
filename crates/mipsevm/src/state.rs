//! This module contains the data structure for the state of the MIPS emulator.

use crate::{Memory, StateWitnessHasher};
use alloy_primitives::{keccak256, Bytes, B256};
use anyhow::Result;

pub const STATE_WITNESS_SIZE: usize = 226;

#[derive(Debug, Default)]
pub struct State {
    /// The [Memory] of the emulated MIPS thread context.
    pub memory: Memory,
    /// The preimage key for the given state.
    pub preimage_key: B256,
    /// The preimage value for the given state.
    pub preimage_value: u32,
    /// The current program counter.
    pub pc: u32,
    /// The next program counter.
    pub next_pc: u32,
    pub lo: u32,
    pub hi: u32,
    pub heap: u32,
    pub exit_code: u8,
    pub exited: bool,
    pub step: u64,
    pub registers: [u32; 32],
    pub last_hint: Bytes,
}

impl State {
    /// Encode the current [State] into a [StateWitness].
    ///
    /// ### Returns
    /// - A [Result] containing the encoded [StateWitness] or an error if the encoding failed.
    pub fn encode_witness(&mut self) -> Result<StateWitness> {
        let mut witness: StateWitness = [0u8; STATE_WITNESS_SIZE];
        witness[..32].copy_from_slice(self.memory.merkle_root()?.as_slice());
        witness[32..64].copy_from_slice(self.preimage_key.as_slice());
        witness[64..68].copy_from_slice(&self.preimage_value.to_be_bytes());
        witness[68..72].copy_from_slice(&self.pc.to_be_bytes());
        witness[72..76].copy_from_slice(&self.next_pc.to_be_bytes());
        witness[76..80].copy_from_slice(&self.lo.to_be_bytes());
        witness[80..84].copy_from_slice(&self.hi.to_be_bytes());
        witness[84..88].copy_from_slice(&self.heap.to_be_bytes());
        witness[88] = self.exit_code;
        witness[89] = self.exited as u8;
        witness[90..98].copy_from_slice(&self.step.to_be_bytes());
        for (i, r) in self.registers.iter().enumerate() {
            let start = 98 + i * 4;
            witness[start..start + 4].copy_from_slice(&r.to_be_bytes());
        }
        Ok(witness)
    }

    /// Return the [VMStatus] given `exited` and `exit_code` statuses.
    pub fn vm_status(exited: bool, exit_code: u8) -> VMStatus {
        if !exited {
            return VMStatus::Unfinished;
        }

        match exit_code {
            0 => VMStatus::Valid,
            1 => VMStatus::Invalid,
            _ => VMStatus::Panic,
        }
    }
}

pub type StateWitness = [u8; STATE_WITNESS_SIZE];

impl StateWitnessHasher for StateWitness {
    fn state_hash(&self) -> B256 {
        let mut hash = keccak256(self);
        let offset = 32 * 2 + 4 * 6;
        let exit_code = self[offset];
        let exited = self[offset + 1];
        hash[0] = State::vm_status(exited == 1, exit_code) as u8;
        hash
    }
}

#[repr(u8)]
pub enum VMStatus {
    Valid = 0,
    Invalid = 1,
    Panic = 2,
    Unfinished = 3,
}

struct InstrumentedState {}
