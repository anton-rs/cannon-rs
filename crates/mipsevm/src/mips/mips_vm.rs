//! This module contains the MIPS VM implementation for the [InstrumentedState].

use crate::{
    memory::MemoryReader,
    mips::instrumented::{MIPS_EBADF, MIPS_EINVAL},
    page,
    types::Syscall,
    Address, Fd, InstrumentedState, PreimageOracle,
};
use anyhow::Result;
use std::io::{self, BufReader, Read, Write};

impl<O, E, P> InstrumentedState<O, E, P>
where
    O: Write,
    E: Write,
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
    #[inline(always)]
    pub(crate) fn read_preimage(
        &mut self,
        key: [u8; 32],
        offset: u32,
    ) -> Result<([u8; 32], usize)> {
        if key != self.last_preimage_key {
            let data = self.preimage_oracle.get(key)?;
            self.last_preimage_key = key;

            // Add the length prefix to the preimage
            // Resizes the `last_preimage` vec in-place to reduce reallocations.
            self.last_preimage.resize(8 + data.len(), 0);
            self.last_preimage[0..8].copy_from_slice(&data.len().to_be_bytes());
            self.last_preimage[8..].copy_from_slice(&data);
        }

        self.last_preimage_offset = offset;

        let mut data = [0u8; 32];
        let data_len =
            BufReader::new(&self.last_preimage[offset as usize..]).read(data.as_mut_slice())?;
        Ok((data, data_len))
    }

    /// Track an access to [crate::Memory] at the given [Address].
    ///
    /// ### Takes
    /// - `effective_address`: The address in [crate::Memory] being accessed.
    ///
    /// ### Returns
    /// - A [Result] indicating if the operation was successful.
    #[inline(always)]
    pub(crate) fn track_mem_access(&mut self, effective_address: Address) -> Result<()> {
        if self.mem_proof_enabled && self.last_mem_access != effective_address {
            if self.last_mem_access != Address::MAX {
                anyhow::bail!("Unexpected diffrent memory access at {:x}, already have access at {:x} buffered", effective_address, self.last_mem_access);
            }

            self.last_mem_access = effective_address;
            self.mem_proof = self.state.memory.merkle_proof(effective_address)?;
        }
        Ok(())
    }

    /// Performs a single step of the MIPS thread context emulation.
    ///
    /// ### Returns
    /// - A [Result] indicating if the step was successful.
    #[inline(always)]
    pub(crate) fn inner_step(&mut self) -> Result<()> {
        if self.state.exited {
            return Ok(());
        }

        self.state.step += 1;

        // Fetch the instruction
        let instruction = self.state.memory.get_memory(self.state.pc as Address)?;
        let opcode = instruction >> 26;

        // j-type j/jal
        if (2..=3).contains(&opcode) {
            let link_reg = if opcode == 3 { 31 } else { 0 };
            // Take the top 4 bits of the next PC (its 256MB region), and concatenate with the
            // 26-bit offset
            let target = self.state.next_pc & 0xF0000000 | ((instruction & 0x03FFFFFF) << 2);
            return self.handle_jump(link_reg, target);
        }

        // Register fetch
        let mut rs = self.state.registers[((instruction >> 21) & 0x1F) as usize]; // source register 1 value
        let mut rt = 0; // source register 2 / temp value
        let rt_reg = (instruction >> 16) & 0x1F;

        // R-type or I-type (stores rt)
        let mut rd_reg = rt_reg;
        if [0, 0x1c].contains(&opcode) {
            // R-type (stores rd)
            rt = self.state.registers[rt_reg as usize];
            rd_reg = (instruction >> 11) & 0x1F;
        } else if opcode < 20 {
            // rt is SignExtImm
            // Don't sign extend for andi, ori, xori
            if (0x0c..=0x0e).contains(&opcode) {
                // ZeroExtImm
                rt = instruction & 0xFFFF;
            } else {
                // SignExtImm
                rt = sign_extend(instruction & 0xFFFF, 16);
            }
        } else if opcode >= 0x28 || [0x22, 0x26].contains(&opcode) {
            // Store rt value with store
            rt = self.state.registers[rt_reg as usize];

            // Store actual rt with lwl and lwr
            rd_reg = rt_reg;
        }

        if (4..8).contains(&opcode) || opcode == 1 {
            return self.handle_branch(opcode, instruction, rt_reg, rs);
        }

        let mut store_address: u32 = 0xFFFFFFFF;
        let mut mem = 0;
        // Memory fetch (all I-type)
        // We also do the load for stores
        if opcode >= 0x20 {
            // M[R[rs]+SignExtImm]
            rs += sign_extend(instruction & 0xFFFF, 16);
            let address = rs & 0xFFFFFFFC;
            self.track_mem_access(address as Address)?;

            mem = self.state.memory.get_memory(address as Address)?;
            if opcode >= 0x28 && opcode != 0x30 {
                // Store
                store_address = address;
                // Store opcodes don't write back to a register
                rd_reg = 0;
            }
        }

        // ALU
        let val = self.execute(instruction, rs, rt, mem)?;

        let fun = instruction & 0x3F;
        if opcode == 0 && (8..0x1c).contains(&fun) {
            match fun {
                (8..=9) => {
                    let link_reg = if fun == 9 { rd_reg } else { 0 };
                    return self.handle_jump(link_reg, rs);
                }
                0x0A => {
                    // movz
                    return self.handle_rd(rd_reg, val, rt == 0);
                }
                0x0B => {
                    // movn
                    return self.handle_rd(rd_reg, val, rt != 0);
                }
                0x0C => {
                    // syscall (can read and write)
                    return self.handle_syscall();
                }
                (0x10..=0x1b) => {
                    // lo and hi registers
                    // Can write back
                    return self.handle_hi_lo(fun, rs, rt, rd_reg);
                }
                _ => {}
            }
        }

        if opcode == 0x38 && rt_reg != 0 {
            self.state.registers[rt_reg as usize] = 1;
        }

        // Write memory
        if store_address != 0xFFFFFFFF {
            self.track_mem_access(store_address as Address)?;
            self.state
                .memory
                .set_memory(store_address as Address, val)?;
        }

        // Write back the value to the destination register
        self.handle_rd(rd_reg, val, true)
    }

    /// Handles a syscall within the MIPS thread context emulation.
    ///
    /// ### Returns
    /// - A [Result] indicating if the syscall dispatch was successful.
    #[inline(always)]
    pub(crate) fn handle_syscall(&mut self) -> Result<()> {
        let mut v0 = 0;
        let mut v1 = 0;

        let (a0, a1, mut a2) = (
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
                        let memory = self.state.memory.get_memory(effective_address)?;

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
                    Ok(fd @ (Fd::Stdout | Fd::StdErr)) => {
                        let mut reader =
                            MemoryReader::new(&mut self.state.memory, a1 as Address, a2);
                        let writer: &mut dyn Write = if matches!(fd, Fd::Stdout) {
                            &mut self.std_out
                        } else {
                            &mut self.std_err
                        };
                        io::copy(&mut reader, writer)?;
                        v0 = a2;
                    }
                    Ok(Fd::HintWrite) => {
                        let mut reader =
                            MemoryReader::new(&mut self.state.memory, a1 as Address, a2);
                        let mut hint_data = Vec::with_capacity(a2 as usize);
                        reader.read_to_end(&mut hint_data)?;
                        self.state.last_hint.extend(hint_data);

                        // Continue processing while there is enough data to check if there are any
                        // hints.
                        while self.state.last_hint.len() >= 4 {
                            let hint_len =
                                u32::from_be_bytes(self.state.last_hint[..4].try_into()?);
                            if hint_len >= self.state.last_hint.len() as u32 - 4 {
                                let hint = &self.state.last_hint[4..4 + hint_len as usize];

                                // TODO(clabby): Ordering could be an issue here.
                                self.preimage_oracle.hint(hint)?;
                                self.state.last_hint =
                                    self.state.last_hint[4 + hint_len as usize..].into();
                            } else {
                                break;
                            }
                        }
                        v0 = a2;
                    }
                    Ok(Fd::PreimageWrite) => {
                        let effective_address = a1 & 0xFFFFFFFC;
                        self.track_mem_access(effective_address as Address)?;

                        let memory = self.state.memory.get_memory(effective_address as Address)?;
                        let mut key = self.state.preimage_key;
                        let alignment = a1 & 0x3;
                        let space = 4 - alignment;

                        if space < a2 {
                            a2 = space;
                        }

                        let key_copy = key;
                        io::copy(
                            &mut key_copy[a2 as usize..].as_ref(),
                            &mut key.as_mut_slice(),
                        )?;

                        let _ = memory.to_be_bytes()[alignment as usize..]
                            .as_ref()
                            .read(&mut key.as_mut_slice()[32 - a2 as usize..])?;

                        self.state.preimage_key = key;
                        self.state.preimage_offset = 0;
                        v0 = a2;
                    }
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
    #[inline(always)]
    pub(crate) fn handle_branch(
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
            self.state.next_pc += 4;
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
    #[inline(always)]
    pub(crate) fn handle_hi_lo(
        &mut self,
        fun: u32,
        rs: u32,
        rt: u32,
        store_reg: u32,
    ) -> Result<()> {
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
                let acc = ((rs as i32) as i64) as u64 * ((rt as i32) as i64) as u64;
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

    /// Handles a jump within the MIPS thread context emulation.
    ///
    /// ### Takes
    /// - `link_reg`: The register index of the link register.
    /// - `dest`: The destination address of the jump.
    ///
    /// ### Returns
    /// - A [Result] indicating if the branch dispatch was successful.
    #[inline(always)]
    pub(crate) fn handle_jump(&mut self, link_reg: u32, dest: u32) -> Result<()> {
        if self.state.next_pc != self.state.pc + 4 {
            anyhow::bail!("Unexpected jump in delay slot at {:x}", self.state.pc);
        }

        let prev_pc = self.state.pc;
        self.state.pc = self.state.next_pc;
        self.state.next_pc = dest;
        if link_reg != 0 {
            self.state.registers[link_reg as usize] = prev_pc + 8;
        }
        Ok(())
    }

    /// Handles a register destination instruction within the MIPS thread context emulation.
    ///
    /// ### Takes
    /// - `store_reg`: The register index of the register to store the result in.
    /// - `val`: The value to store in the register.
    /// - `conditional`: Whether or not the register should be updated.
    ///
    /// ### Returns
    /// - A [Result] indicating if the branch dispatch was successful.
    #[inline(always)]
    pub(crate) fn handle_rd(&mut self, store_reg: u32, val: u32, conditional: bool) -> Result<()> {
        if store_reg >= 32 {
            anyhow::bail!("Invalid register index {}", store_reg);
        }

        if store_reg != 0 && conditional {
            self.state.registers[store_reg as usize] = val;
        }

        self.state.pc = self.state.next_pc;
        self.state.next_pc += 4;
        Ok(())
    }

    /// Handles the execution of a MIPS instruction in the MIPS thread context emulation.
    ///
    /// ### Takes
    /// - `instruction`: The instruction to execute.
    /// - `rs`: The register index of the source register.
    /// - `rt`: The register index of the target register.
    /// - `mem`: The memory that the instruction is operating on.
    ///
    /// ### Returns
    /// - `Ok(n)` - The result of the instruction execution.
    /// - `Err(_)`: An error occurred while executing the instruction.
    #[inline(always)]
    pub(crate) fn execute(&mut self, instruction: u32, rs: u32, rt: u32, mem: u32) -> Result<u32> {
        // Opcodes in MIPS are 6 bits in size, and stored in the high-order bits of the big-endian
        // instruction.
        let opcode = instruction >> 26;

        if opcode == 0 || (8..0xF).contains(&opcode) {
            let fun = match opcode {
                // addi
                8 => 0x20,
                // addiu
                9 => 0x21,
                // slti
                0xA => 0x2A,
                // sltiu
                0xB => 0x2B,
                // andi
                0xC => 0x24,
                // ori
                0xD => 0x25,
                // xori
                0xE => 0x26,
                _ => instruction & 0x3F,
            };

            match fun {
                // sll
                0 => Ok(rt << ((instruction >> 6) & 0x1F)),
                // srl
                2 => Ok(rt >> ((instruction >> 6) & 0x1F)),
                // sra
                3 => {
                    let shamt = (instruction >> 6) & 0x1F;
                    Ok(sign_extend(rt >> shamt, 32 - shamt))
                }
                // sslv
                4 => Ok(rt << (rs & 0x1F)),
                // srlv
                6 => Ok(rt >> (rs & 0x1F)),
                7 => Ok(sign_extend(rt >> rs, 32 - rs)),

                // Functions in range [0x8, 0x1b] are handled specially by other functions.

                // jr, jalr, movz, movn, syscall, sync, mfhi, mthi, mflo, mftlo, mult, multu, div,
                // divu
                (8..=0x0c) | (0x0f..=0x13) | (0x18..=0x1b) => Ok(rs),

                // The rest are transformed R-type arithmetic imm instructions.

                // add / addu
                0x20 | 0x21 => Ok(rs + rt),
                // sub / subu
                0x22 | 0x23 => Ok(rs - rt),
                // and
                0x24 => Ok(rs & rt),
                // or
                0x25 => Ok(rs | rt),
                // xor
                0x26 => Ok(rs ^ rt),
                // nor
                0x27 => Ok(!(rs | rt)),
                // slti
                0x2a => Ok(((rs as i32) < (rt as i32)) as u32),
                // sltiu
                0x2b => Ok((rs < rt) as u32),
                _ => anyhow::bail!("Invalid function code {:x}", fun),
            }
        } else {
            match opcode {
                // SPECIAL2
                0x1C => {
                    let fun = instruction & 0x3F;
                    match fun {
                        // mul
                        0x02 => Ok(((rs as i32) * (rt as i32)) as u32),
                        // clo
                        0x20 | 0x21 => {
                            let mut rs = rs;
                            if fun == 0x20 {
                                rs = !rs;
                            }
                            let mut i = 0u32;

                            // TODO(clabby): Remove loop, do some good ol' bit twiddling instead.
                            while rs & 0x80000000 != 0 {
                                rs <<= 1;
                                i += 1;
                            }
                            Ok(i)
                        }
                        _ => anyhow::bail!("Invalid function code {:x}", fun),
                    }
                }
                // lui
                0x0F => Ok(rt << 16),
                // lb
                0x20 => Ok(sign_extend((mem >> (24 - ((rs & 0x3) << 3))) & 0xFF, 8)),
                // lh
                0x21 => Ok(sign_extend((mem >> (16 - ((rs & 0x2) << 3))) & 0xFFFF, 16)),
                // lwl
                0x22 => {
                    let sl = (rs & 0x3) << 3;
                    let val = mem << sl;
                    let mask = 0xFFFFFFFF << sl;
                    Ok((rt & !mask) | val)
                }
                // lw
                0x23 => Ok(mem),
                // lbu
                0x24 => Ok((mem >> (24 - ((rs & 0x3) << 3))) & 0xFF),
                // lhu
                0x25 => Ok((mem >> (16 - ((rs & 0x2) << 3))) & 0xFFFF),
                // lwr
                0x26 => {
                    let sr = 24 - ((rs & 0x3) << 3);
                    let val = mem >> sr;
                    let mask = 0xFFFFFFFFu32 >> sr;
                    Ok((rt & !mask) | val)
                }
                // sb
                0x28 => {
                    let sl = 24 - ((rs & 0x3) << 3);
                    let val = (rt & 0xFF) << sl;
                    let mask = 0xFFFFFFFF ^ (0xFF << sl);
                    Ok((mem & mask) | val)
                }
                // sh
                0x29 => {
                    let sl = 16 - ((rs & 0x2) << 3);
                    let val = (rt & 0xFFFF) << sl;
                    let mask = 0xFFFFFFFF ^ (0xFFFF << sl);
                    Ok((mem & mask) | val)
                }
                // swl
                0x2a => {
                    let sr = (rs & 0x3) << 3;
                    let val = rt >> sr;
                    let mask = 0xFFFFFFFFu32 >> sr;
                    Ok((mem & !mask) | val)
                }
                // sw
                0x2b => Ok(rt),
                // swr
                0x2e => {
                    let sl = 24 - ((rs & 0x3) << 3);
                    let val = rt << sl;
                    let mask = 0xFFFFFFFF << sl;
                    Ok((mem & !mask) | val)
                }
                // ll
                0x30 => Ok(mem),
                // sc
                0x38 => Ok(rt),
                _ => anyhow::bail!("Invalid opcode {:x}", opcode),
            }
        }
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
#[inline(always)]
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
