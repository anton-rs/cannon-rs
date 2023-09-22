//! This module contains the [InstrumentedState] definition.

use crate::{traits::PreimageOracle, Address, State, StepWitness};
use alloy_primitives::B256;
use anyhow::Result;
use std::io::{BufWriter, Write};

pub(crate) const MIPS_EBADF: u32 = 0x9;
pub(crate) const MIPS_EINVAL: u32 = 0x16;

pub struct InstrumentedState<W: Write, P: PreimageOracle> {
    /// The inner [State] of the MIPS thread context.
    pub(crate) state: State,
    /// The MIPS thread context's stdout buffer.
    /// TODO(clabby): Prob not the best place for this.
    pub(crate) std_out: BufWriter<W>,
    /// The MIPS thread context's stderr buffer.
    /// TODO(clabby): Prob not the best place for this.
    pub(crate) std_err: BufWriter<W>,
    /// The last address we accessed in memory.
    pub(crate) last_mem_access: Address,
    /// Whether or not the memory proof generation is enabled.
    pub(crate) mem_proof_enabled: bool,
    /// The memory proof, if it is enabled.
    pub(crate) mem_proof: [u8; 28 * 32],
    /// The [PreimageOracle] used to fetch preimages.
    pub(crate) preimage_oracle: P,
    /// Cached pre-image data, including 8 byte length prefix
    pub(crate) last_preimage: Vec<u8>,
    /// Key for the above preimage
    pub(crate) last_preimage_key: B256,
    /// The offset we last read from, or max u32 if nothing is read at
    /// the current step.
    pub(crate) last_preimage_offset: u32,
}

impl<W, P> InstrumentedState<W, P>
where
    W: Write,
    P: PreimageOracle,
{
    pub fn new(state: State, oracle: P, std_out: W, std_in: W) -> Self {
        Self {
            state,
            std_out: BufWriter::new(std_out),
            std_err: BufWriter::new(std_in),
            last_mem_access: 0,
            mem_proof_enabled: false,
            mem_proof: [0; 28 * 32],
            preimage_oracle: oracle,
            last_preimage: Vec::default(),
            last_preimage_key: B256::default(),
            last_preimage_offset: 0,
        }
    }

    /// Step the MIPS emulator forward one instruction.
    ///
    /// ### Returns
    /// - Ok(Some(witness)): The [StepWitness] for the current step.
    /// - Err(_): An error occurred while processing the instruction step in the MIPS emulator.
    pub fn step(&mut self, proof: bool) -> Result<Option<StepWitness>> {
        self.mem_proof_enabled = proof;
        self.last_mem_access = !0u32 as u64;
        self.last_preimage_offset = !0u32;

        let mut witness = None;
        if proof {
            let instruction_proof = self
                .state
                .memory
                .borrow_mut()
                .merkle_proof(self.state.pc as Address)?;
            witness = Some(StepWitness {
                state: self.state.encode_witness()?,
                mem_proof: instruction_proof.to_vec(),
                preimage_key: B256::ZERO,
                preimage_value: Vec::default(),
                preimage_offset: 0,
            })
        }

        self.mips_step()?;

        if proof {
            witness = witness.map(|mut wit| {
                wit.mem_proof.extend_from_slice(self.mem_proof.as_slice());
                if self.last_preimage_offset != u32::MAX {
                    wit.preimage_key = self.last_preimage_key;
                    wit.preimage_value = self.last_preimage.clone();
                    wit.preimage_offset = self.last_preimage_offset;
                }
                wit
            })
        }

        Ok(witness)
    }
}
