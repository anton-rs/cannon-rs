//! This module contains the MIPS VM implementation for the [InstrumentedState].

use crate::{Address, InstrumentedState, PreimageOracle};
use alloy_primitives::B256;
use anyhow::Result;
use std::io::{Cursor, Read, Write};

impl<W, P> InstrumentedState<W, P>
where
    W: Write,
    P: PreimageOracle,
{
    /// Read the preimage for the given key and offset from the [PreimageOracle] server.
    ///
    /// ### Takes
    /// - `key`: The key of the preimage (the preimage's [alloy_primitives::keccak256] digest).
    /// - `offset`: The offset of the preimage to fetch.
    ///
    /// ### Returns
    /// - `Ok((data, data_len))`: The preimage data and length.
    /// - `Err(_)`: An error occurred while fetching the preimage.
    pub fn read_preimage(&mut self, key: B256, offset: u32) -> Result<(B256, usize)> {
        if key != self.last_preimage_key {
            self.last_preimage_key = key;
            let data = self.preimage_oracle.get(key)?;

            // Add the length prefix to the preimage
            // Resizes the `last_preimage` vec in-place to reduce reallocations.
            self.last_preimage.resize(8 + data.len(), 0);
            self.last_preimage[0..8].copy_from_slice(&data.len().to_be_bytes());
            self.last_preimage[8..].copy_from_slice(data);
        }

        self.last_preimage_offset = offset;

        // TODO(clabby): This could be problematic if the `Cursor`'s read function returns
        // 0 as EOF rather than the amount of bytes read into `data`.
        let mut data = B256::ZERO;
        let data_len =
            Cursor::new(&self.last_preimage[offset as usize..]).read(data.as_mut_slice())?;

        Ok((data, data_len))
    }

    /// Track an access to [crate::Memory] at the given [Address].
    ///
    /// ### Takes
    /// - `effective_address`: The address in [crate::Memory] being accessed.
    ///
    /// ### Returns
    /// - A [Result] indicating if the operation was successful.
    pub fn track_mem_access(&mut self, effective_address: Address) -> Result<()> {
        if self.mem_proof_enabled && self.last_mem_access != effective_address {
            if self.last_mem_access != Address::MAX {
                anyhow::bail!("Unexpected diffrent memory access at {:x}, already have access at {:x} buffered", effective_address, self.last_mem_access);
            }

            self.last_mem_access = effective_address;
            self.mem_proof = self.state.memory.merkle_proof(effective_address)?;
        }
        Ok(())
    }

    /// Handles a syscall within the MIPS thread context emulation.
    ///
    /// ### Returns
    /// - A [Result] indicating if the syscall dispatch was successful.
    pub fn handle_syscall(&mut self) -> Result<()> {
        todo!()
    }
}

/// Perform a sign extension of a value embedded in the lower bits of `data` up to
/// the `index`th bit.
///
/// ### Takes
/// - `data`: The data to sign extend.
/// - `index`: The index of the bit to sign extend to.
///
/// ### Returns
/// - The sign extended value.
pub(crate) fn sign_extend(data: u32, index: u32) -> u32 {
    let is_signed = (data >> (index - 1)) != 0;
    let signed = ((1 << (32 - index)) - 1) << index;
    let mask = (1 << index) - 1;
    if is_signed {
        (data & mask) | signed
    } else {
        data & mask
    }
}
