//! This module contains the data structure for the state of the MIPS emulator.

use std::{cell::RefCell, rc::Rc};

use crate::{witness::STATE_WITNESS_SIZE, Memory, StateWitness, VMStatus};
use alloy_primitives::B256;
use anyhow::Result;

#[derive(Debug, Default)]
pub struct State {
    /// The [Memory] of the emulated MIPS thread context.
    pub memory: Rc<RefCell<Memory>>,
    /// The preimage key for the given state.
    pub preimage_key: B256,
    /// The preimage offset.
    pub preimage_offset: u32,
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
    pub last_hint: Vec<u8>,
}

impl State {
    /// Encode the current [State] into a [StateWitness].
    ///
    /// ### Returns
    /// - A [Result] containing the encoded [StateWitness] or an error if the encoding failed.
    pub fn encode_witness(&mut self) -> Result<StateWitness> {
        let mut witness: StateWitness = [0u8; STATE_WITNESS_SIZE];
        witness[..32].copy_from_slice(self.memory.borrow_mut().merkle_root()?.as_slice());
        witness[32..64].copy_from_slice(self.preimage_key.as_slice());
        witness[64..68].copy_from_slice(&self.preimage_offset.to_be_bytes());
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
