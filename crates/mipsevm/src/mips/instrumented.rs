//! This module contains the [InstrumentedState] definition.

use crate::{traits::PreimageOracle, Address, State, StepWitness};
use alloy_primitives::B256;
use anyhow::Result;
use std::io::{BufWriter, Write};

pub(crate) const MIPS_EBADF: u32 = 0x9;
pub(crate) const MIPS_EINVAL: u32 = 0x16;

pub struct InstrumentedState<O: Write, E: Write, P: PreimageOracle> {
    /// The inner [State] of the MIPS thread context.
    pub(crate) state: State,
    /// The MIPS thread context's stdout buffer.
    /// TODO(clabby): Prob not the best place for this.
    pub(crate) std_out: BufWriter<O>,
    /// The MIPS thread context's stderr buffer.
    /// TODO(clabby): Prob not the best place for this.
    pub(crate) std_err: BufWriter<E>,
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

impl<O, E, P> InstrumentedState<O, E, P>
where
    O: Write,
    E: Write,
    P: PreimageOracle,
{
    pub fn new(state: State, oracle: P, std_out: O, std_err: E) -> Self {
        Self {
            state,
            std_out: BufWriter::new(std_out),
            std_err: BufWriter::new(std_err),
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
    /// - Ok(Some(witness)): The [StepWitness] for the current
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

        self.inner_step()?;

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

#[cfg(test)]
mod test {
    use crate::PreimageOracle;

    /// Used in tests to write the results to
    const BASE_ADDR_END: u32 = 0xBF_FF_FF_F0;

    /// Used as the return-address for tests
    const END_ADDR: u32 = 0xA7_EF_00_D0;

    struct StaticOracle;

    impl PreimageOracle for StaticOracle {
        fn hint(&mut self, _value: &[u8]) {
            // noop
        }

        fn get(&self, _key: alloy_primitives::B256) -> anyhow::Result<&[u8]> {
            // noop
            Ok(&[])
        }
    }

    mod open_mips {
        use super::*;
        use crate::{Address, InstrumentedState, Memory, State};
        use std::{
            cell::RefCell,
            fs,
            io::{self, BufReader},
            path::PathBuf,
            rc::Rc,
        };

        #[test]
        // #[ignore]
        fn open_mips_tests() {
            let tests_path = PathBuf::from(std::env::current_dir().unwrap())
                .join("open_mips_tests")
                .join("test")
                .join("bin");
            let test_files = fs::read_dir(tests_path).unwrap();

            for f in test_files.into_iter() {
                if let Ok(f) = f {
                    let file_name = String::from(f.file_name().to_str().unwrap());
                    if file_name.starts_with("oracle") {
                        dbg!("Skipping oracle test");
                        continue;
                    }

                    // Short circuit early for `exit_group.bin`
                    let exit_group = file_name == "exit_group.bin";

                    let program_mem = fs::read(f.path()).unwrap();

                    let mut state = {
                        let mut state = State::default();
                        state.pc = 0;
                        state.next_pc = 4;
                        state.memory = Rc::new(RefCell::new(Memory::default()));
                        state
                    };
                    state
                        .memory
                        .borrow_mut()
                        .set_memory_range(0, BufReader::new(program_mem.as_slice()))
                        .unwrap();

                    // Set the return address ($ra) to jump into when the test completes.
                    state.registers[31] = END_ADDR;

                    let mut ins =
                        InstrumentedState::new(state, StaticOracle {}, io::stdout(), io::stderr());

                    for _ in 0..1000 {
                        if ins.state.pc == END_ADDR {
                            break;
                        }
                        if exit_group && ins.state.exited {
                            break;
                        }
                        ins.step(false).unwrap();
                    }

                    if exit_group {
                        assert_ne!(END_ADDR, ins.state.pc, "must not reach end");
                        assert!(ins.state.exited, "must exit");
                        assert_eq!(1, ins.state.exit_code, "must exit with 1");
                    } else {
                        assert_eq!(END_ADDR, ins.state.pc, "must reach end");
                        let mut state = ins.state.memory.borrow_mut();
                        let (done, result) = (
                            state.get_memory((BASE_ADDR_END + 4) as Address).unwrap(),
                            state.get_memory((BASE_ADDR_END + 8) as Address).unwrap(),
                        );
                        assert_eq!(done, 1, "must set done to 1");
                        assert_eq!(result, 1, "must have success result {:?}", f.file_name());
                    }
                }
            }
        }
    }
}
