//! This module contains the MIPS VM implementation for the [InstrumentedState].

use crate::{
    memory::MemoryReader,
    mips::instrumented::{MIPS_EBADF, MIPS_EINVAL},
    page,
    types::Syscall,
    Address, Fd, InstrumentedState, PreimageOracle,
};
use alloy_primitives::B256;
use anyhow::Result;
use std::{
    io::{Cursor, Read, Write},
    rc::Rc,
};

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
            self.mem_proof = self
                .state
                .memory
                .borrow_mut()
                .merkle_proof(effective_address)?;
        }
        Ok(())
    }

    /// Handles a syscall within the MIPS thread context emulation.
    ///
    /// ### Returns
    /// - A [Result] indicating if the syscall dispatch was successful.
    pub fn handle_syscall(&mut self) -> Result<()> {
        let mut v0 = 0;
        let mut v1 = 0;

        let (a0, a1, a2) = (
            self.state.registers[4],
            self.state.registers[5],
            self.state.registers[6],
        );

        if let Ok(syscall) = Syscall::try_from(self.state.registers[2]) {
            match syscall {
                Syscall::Mmap => {
                    let mut sz = a1;

                    // Adjust the size to align with the page size if the size
                    // cannot fit within the page address mask.
                    let masked_size = sz & page::PAGE_ADDRESS_MASK as u32;
                    if masked_size != 0 {
                        sz += page::PAGE_SIZE as u32 - masked_size;
                    }

                    if a0 == 0 {
                        v0 = self.state.heap;
                        self.state.heap += sz;
                    } else {
                        v0 = a0;
                    }
                }
                Syscall::Brk => {
                    v0 = 0x40000000;
                }
                Syscall::Clone => {
                    // Clone is not supported, set the virtual register to 1.
                    v0 = 1;
                }
                Syscall::ExitGroup => {
                    self.state.exited = true;
                    self.state.exit_code = a0 as u8;
                    return Ok(());
                }
                Syscall::Read => match (a0 as u8).try_into() {
                    Ok(Fd::StdIn) => {
                        // Nothing to do; Leave v0 and v1 zero, read nothing, and give no error.
                    }
                    Ok(Fd::PreimageRead) => {
                        let effective_address = (a1 & 0xFFFFFFFC) as Address;

                        self.track_mem_access(effective_address)?;
                        let memory = self
                            .state
                            .memory
                            .borrow_mut()
                            .get_memory(effective_address)?;

                        let (data, mut data_len) = self
                            .read_preimage(self.state.preimage_key, self.state.preimage_offset)?;

                        let alignment = (a1 & 0x3) as usize;
                        let space = 4 - alignment;
                        if space < data_len {
                            data_len = space;
                        }
                        if (a2 as usize) < data_len {
                            data_len = a2 as usize;
                        }

                        let mut out_mem = memory.to_be_bytes();
                        out_mem[alignment..alignment + data_len].copy_from_slice(&data[..data_len]);
                        self.state
                            .memory
                            .borrow_mut()
                            .set_memory(effective_address, u32::from_be_bytes(out_mem))?;
                        self.state.preimage_offset += data_len as u32;
                        v0 = data_len as u32;
                    }
                    Ok(Fd::HintRead) => {
                        // Don't actually read anything into memory, just say we read it. The
                        // result is ignored anyways.
                        v0 = a2;
                    }
                    _ => {
                        v0 = 0xFFFFFFFF;
                        v1 = MIPS_EBADF;
                    }
                },
                Syscall::Write => match (a0 as u8).try_into() {
                    Ok(Fd::Stdout) => {
                        let _reader = MemoryReader::new(
                            Rc::clone(&self.state.memory),
                            a1 as Address,
                            a2 as u64,
                        );
                        todo!()
                    }
                    Ok(Fd::StdErr) => {
                        let _reader = MemoryReader::new(
                            Rc::clone(&self.state.memory),
                            a1 as Address,
                            a2 as u64,
                        );
                        todo!()
                    }
                    Ok(Fd::HintWrite) => {}
                    Ok(Fd::PreimageWrite) => {}
                    _ => {
                        v0 = 0xFFFFFFFF;
                        v1 = MIPS_EBADF;
                    }
                },
                Syscall::Fcntl => {
                    if a1 == 3 {
                        match (a0 as u8).try_into() {
                            Ok(Fd::StdIn | Fd::PreimageRead | Fd::HintRead) => {
                                v0 = 0; // O_RDONLY
                            }
                            Ok(Fd::Stdout | Fd::StdErr | Fd::PreimageWrite | Fd::HintWrite) => {
                                v0 = 1; // O_WRONLY
                            }
                            _ => {
                                v0 = 0xFFFFFFFF;
                                v1 = MIPS_EBADF;
                            }
                        }
                    } else {
                        // The command is not recognized by this kernel.
                        v0 = 0xFFFFFFFF;
                        v1 = MIPS_EINVAL;
                    }
                }
            }
        }

        self.state.registers[2] = v0;
        self.state.registers[7] = v1;

        self.state.pc = self.state.next_pc;
        self.state.next_pc += 4;

        Ok(())
    }

    /// Handles a branch within the MIPS thread context emulation.
    ///
    /// ### Takes
    /// - `opcode`: The opcode of the branch instruction.
    /// - `instruction`: The instruction being executed.
    /// - `rt_reg`: The register index of the target register.
    /// - `rs`: The register index of the source register.
    ///
    /// ### Returns
    /// - A [Result] indicating if the branch dispatch was successful.
    pub fn handle_branch(
        &mut self,
        opcode: u32,
        instruction: u32,
        rt_reg: u32,
        rs: u32,
    ) -> Result<()> {
        if self.state.next_pc != self.state.pc + 4 {
            anyhow::bail!("Unexpected branch in delay slot at {:x}", self.state.pc,);
        }

        let should_branch = match opcode {
            // beq / bne
            4 | 5 => {
                let rt = self.state.registers[rt_reg as usize];
                (rs == rt && opcode == 4) || (rs != rt && opcode == 5)
            }
            // blez
            6 => (rs as i32) <= 0,
            // bgtz
            7 => (rs as i32) > 0,
            1 => {
                // regimm
                let rtv = (instruction >> 16) & 0x1F;

                if rtv == 0 {
                    // bltz
                    (rs as i32) < 0
                } else if rtv == 1 {
                    // bgez
                    (rs as i32) >= 0
                } else {
                    false
                }
            }
            _ => false,
        };

        let prev_pc = self.state.pc;
        self.state.pc = self.state.next_pc;

        if should_branch {
            self.state.next_pc = prev_pc + 4 + (sign_extend(instruction & 0xFFFF, 16) << 2);
        } else {
            // Branch not taken; proceed as normal.
            self.state.next_pc = self.state.next_pc + 4;
        }

        Ok(())
    }

    /// Handles a hi/lo instruction within the MIPS thread context emulation.
    ///
    /// ### Takes
    /// - `fun`: The function code of the instruction.
    /// - `rs`: The register index of the source register.
    /// - `rt`: The register index of the target register.
    /// - `store_reg`: The register index of the register to store the result in.
    ///
    /// ### Returns
    /// - A [Result] indicating if the branch dispatch was successful.
    pub fn handle_hi_lo(&mut self, fun: u32, rs: u32, rt: u32, store_reg: u32) -> Result<()> {
        let val = match fun {
            0x10 => {
                // mfhi
                self.state.hi
            }
            0x11 => {
                // mthi
                self.state.hi = rs;
                0
            }
            0x12 => {
                // mflo
                self.state.lo
            }
            0x13 => {
                // mtlo
                self.state.lo = rs;
                0
            }
            0x18 => {
                // mult
                let acc = (rs as i64) as u64 * (rt as i64) as u64;
                self.state.hi = (acc >> 32) as u32;
                self.state.lo = acc as u32;
                0
            }
            0x19 => {
                // multu
                let acc = rs as u64 * rt as u64;
                self.state.hi = (acc >> 32) as u32;
                self.state.lo = acc as u32;
                0
            }
            0x1a => {
                // div
                self.state.hi = (rs as i32 % rt as i32) as u32;
                self.state.lo = (rs as i32 / rt as i32) as u32;
                0
            }
            0x1b => {
                // divu
                self.state.hi = rs % rt;
                self.state.lo = rs / rt;
                0
            }
            _ => 0,
        };

        if store_reg != 0 {
            self.state.registers[store_reg as usize] = val;
        }

        self.state.pc = self.state.next_pc;
        self.state.next_pc += 4;

        Ok(())
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
