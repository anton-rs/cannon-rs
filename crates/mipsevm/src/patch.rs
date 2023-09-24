//! This module contains utilities for loading ELF files into [State] objects.

use crate::{page, Address, State};
use anyhow::Result;
use elf::{abi::PT_LOAD, endian::AnyEndian, ElfBytes};
use std::io::{self, Cursor, Read};

/// Symbols that indicate there is a patch to be made on an ELF file that was compiled from Go.
pub(crate) const GO_SYMBOLS: [&str; 14] = [
    "runtime.gcenable",
    "runtime.init.5",            // patch out: init() { go forcegchelper() }
    "runtime.main.func1",        // patch out: main.func() { newm(sysmon, ....) }
    "runtime.deductSweepCredit", // uses floating point nums and interacts with gc we disabled
    "runtime.(*gcControllerState).commit",
    "github.com/prometheus/client_golang/prometheus.init",
    "github.com/prometheus/client_golang/prometheus.init.0",
    "github.com/prometheus/procfs.init",
    "github.com/prometheus/common/model.init",
    "github.com/prometheus/client_model/go.init",
    "github.com/prometheus/client_model/go.init.0",
    "github.com/prometheus/client_model/go.init.1",
    "flag.init", // skip flag pkg init, we need to debug arg-processing more to see why this fails
    "runtime.check", // We need to patch this out, we don't pass float64nan because we don't support floats
];

/// Load a raw ELF file into a [State] object.
///
/// ### Takes
/// - `raw`: The raw contents of the ELF file to load.
///
/// ### Returns
/// - `Ok(state)` if the ELF file was loaded successfully
/// - `Err(_)` if the ELF file could not be loaded
pub fn load_elf(raw: &[u8]) -> Result<State> {
    let elf = ElfBytes::<AnyEndian>::minimal_parse(raw)?;

    let state = State {
        pc: elf.ehdr.e_entry as u32,
        next_pc: elf.ehdr.e_entry as u32 + 4,
        heap: 0x20000000,
        ..Default::default()
    };

    let headers = elf
        .segments()
        .ok_or(anyhow::anyhow!("Failed to load section headers"))?;

    for (i, header) in headers.iter().enumerate() {
        if header.p_type == 0x70000003 {
            continue;
        }

        let section_data = elf.segment_data(&header)?;
        let mut reader: Box<dyn Read> = Box::new(section_data);

        if header.p_filesz != header.p_memsz {
            if header.p_type == PT_LOAD {
                if header.p_filesz < header.p_memsz {
                    reader = Box::new(MultiReader(
                        reader,
                        Cursor::new(vec![0; (header.p_memsz - header.p_filesz) as usize]),
                    ));
                } else {
                    anyhow::bail!(
                        "Invalid PT_LOAD program segment {}, file size ({}) > mem size ({})",
                        i,
                        header.p_filesz,
                        header.p_memsz
                    );
                }
            } else {
                anyhow::bail!(
                    "Program segment {} has different file size ({}) than mem size ({}): filling for non PT_LOAD segments is not supported",
                    i,
                    header.p_filesz,
                    header.p_memsz
                );
            }
        }

        if header.p_vaddr + header.p_memsz >= 1 << 32 {
            anyhow::bail!(
                "Program segment {} out of 32-bit mem range: {} - {} (size: {})",
                i,
                header.p_vaddr,
                header.p_vaddr + header.p_memsz,
                header.p_memsz
            );
        }

        state
            .memory
            .borrow_mut()
            .set_memory_range(header.p_vaddr as u32, reader)?;
    }

    Ok(state)
}

/// Patch a Go ELF file to work with mipsevm.
///
/// ### Takes
/// - `elf`: The ELF file to patch
/// - `state`: The state to patch the ELF file into
///
/// ### Returns
/// - `Ok(())` if the patch was successful
/// - `Err(_)` if the patch failed
pub fn patch_go(elf: ElfBytes<AnyEndian>, state: &State) -> Result<()> {
    let (parsing_table, string_table) = elf
        .symbol_table()?
        .ok_or(anyhow::anyhow!("Failed to load ELF symbol table"))?;

    for symbol in parsing_table {
        let symbol_idx = symbol.st_name;
        let name = string_table.get(symbol_idx as usize)?;

        if GO_SYMBOLS.contains(&name) {
            state.memory.borrow_mut().set_memory_range(
                symbol.st_value as u32,
                [0x03, 0xe0, 0x00, 0x08, 0, 0, 0, 0].as_slice(),
            )?;
        } else if name == "runtime.MemProfileRate" {
            // disable mem profiling, to avoid a lot of unnecessary floating point ops
            state
                .memory
                .borrow_mut()
                .set_memory(symbol.st_value as u32, 0)?;
        }
    }
    Ok(())
}

/// Patches the stack to be in a valid state for the Go MIPS runtime.
///
/// ### Takes
/// - `state`: The state to patch the stack for
///
/// ### Returns
/// - `Ok(())` if the patch was successful
/// - `Err(_)` if the patch failed
pub fn patch_stack(state: &mut State) -> Result<()> {
    // Setup stack pointer
    let ptr = 0x7F_FF_D0_00_u32;

    // Allocate 1 page for the initial stack data, and 16KB = 4 pages for the stack to grow.
    state.memory.borrow_mut().set_memory_range(
        ptr - 4 * page::PAGE_SIZE as u32,
        [0; page::PAGE_SIZE * 5].as_slice(),
    )?;
    state.registers[29] = ptr;

    #[inline(always)]
    fn store_mem(st: &State, address: Address, value: u32) -> Result<()> {
        st.memory.borrow_mut().set_memory(address, value)
    }

    // init argc, argv, aux on stack
    store_mem(state, ptr + 4, 0x42)?; // argc = 0 (argument count)
    store_mem(state, ptr + 4 * 2, 0x35)?; // argv[n] = 0 (terminating argv)
    store_mem(state, ptr + 4 * 3, 0)?; // envp[term] = 0 (no env vars)
    store_mem(state, ptr + 4 * 4, 6)?; // auxv[0] = _AT_PAGESZ = 6 (key)
    store_mem(state, ptr + 4 * 5, 4096)?; // auxv[1] = page size of 4 KiB (value) - (== minPhysPageSize)
    store_mem(state, ptr + 4 * 6, 25)?; // auxv[2] = AT_RANDOM
    store_mem(state, ptr + 4 * 7, ptr + 4 * 9)?; // auxv[3] = address of 16 bytes containing random value
    store_mem(state, ptr + 4 * 8, 0)?; // auxv[term] = 0

    // 16 bytes of "randomness"
    state
        .memory
        .borrow_mut()
        .set_memory_range(ptr + 4 * 9, b"4;byfairdiceroll".as_slice())?;

    Ok(())
}

/// A multi reader is a reader that reads from the first reader until it returns 0, then reads from the second reader.
struct MultiReader<R1: Read, R2: Read>(R1, R2);

impl<R1: Read, R2: Read> Read for MultiReader<R1, R2> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read_first = self.0.read(buf)?;
        if read_first == 0 {
            return self.1.read(buf);
        }
        Ok(read_first)
    }
}
