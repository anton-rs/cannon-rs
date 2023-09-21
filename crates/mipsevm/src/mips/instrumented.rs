//! This module contains the [InstrumentedState] definition.

use crate::{traits::PreimageOracle, State};
use alloy_primitives::{Bytes, B256};
use std::io::{BufWriter, Write};

const MIPS_EBADF: u8 = 0x9;
const MIPS_EINVAL: u8 = 0x16;

pub struct InstrumentedState<W: Write, P: PreimageOracle> {
    /// The inner [State] of the MIPS thread context.
    state: State,
    /// The MIPS thread context's stdout buffer.
    /// TODO(clabby): Prob not the best place for this.
    std_out: BufWriter<W>,
    /// The MIPS thread context's stderr buffer.
    /// TODO(clabby): Prob not the best place for this.
    std_err: BufWriter<W>,
    /// The last address we accessed in memory.
    last_mem_access: u32,
    /// Whether or not the memory proof generation is enabled.
    mem_proof_enabled: bool,
    /// The memory proof, if it is enabled.
    mem_proof: [u8; 28 * 32],
    /// The [PreimageOracle] used to fetch preimages.
    preimage_oracle: P,
    /// Cached pre-image data, including 8 byte length prefix
    last_preimage: Bytes,
    /// Key for the above preimage
    last_preimage_key: B256,
    /// The offset we last read from, or max u32 if nothing is read at
    /// the current step.
    last_preimage_offset: u32,
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
            last_preimage: Bytes::default(),
            last_preimage_key: B256::default(),
            last_preimage_offset: 0,
        }
    }
}
