//! This module contains the [InstrumentedState] definition.

use crate::{traits::PreimageOracle, Address, State, StepWitness};
use anyhow::Result;
use std::io::{BufWriter, Write};

pub(crate) const MIPS_EBADF: u32 = 0x9;
pub(crate) const MIPS_EINVAL: u32 = 0x16;

/// The [InstrumentedState] is a wrapper around [State] that contains cached machine state,
/// the input and output buffers, and an implementation of the MIPS VM.
///
/// To perform an instruction step on the MIPS emulator, use the [InstrumentedState::step] method.
pub struct InstrumentedState<O: Write, E: Write, P: PreimageOracle> {
    /// The inner [State] of the MIPS thread context.
    pub state: State,
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
    pub(crate) last_preimage_key: [u8; 32],
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
            mem_proof: [0u8; 28 * 32],
            preimage_oracle: oracle,
            last_preimage: Vec::default(),
            last_preimage_key: [0u8; 32],
            last_preimage_offset: 0,
        }
    }

    /// Step the MIPS emulator forward one instruction.
    ///
    /// ### Returns
    /// - Ok(Some(witness)): The [StepWitness] for the current
    /// - Err(_): An error occurred while processing the instruction step in the MIPS emulator.
    #[inline(always)]
    pub fn step(&mut self, proof: bool) -> Result<Option<StepWitness>> {
        self.mem_proof_enabled = proof;
        self.last_mem_access = !0u32 as Address;
        self.last_preimage_offset = !0u32;

        let mut witness = None;
        if proof {
            let instruction_proof = self.state.memory.merkle_proof(self.state.pc as Address)?;

            let mut mem_proof = vec![0; 28 * 32 * 2];
            mem_proof[0..28 * 32].copy_from_slice(instruction_proof.as_slice());
            witness = Some(StepWitness {
                state: self.state.encode_witness()?,
                mem_proof,
                ..Default::default()
            })
        }

        self.inner_step()?;

        if proof {
            witness = witness.map(|mut wit| {
                wit.mem_proof[28 * 32..].copy_from_slice(self.mem_proof.as_slice());
                if self.last_preimage_offset != u32::MAX {
                    wit.preimage_key = Some(self.last_preimage_key);
                    wit.preimage_value = Some(self.last_preimage.clone());
                    wit.preimage_offset = Some(self.last_preimage_offset);
                }
                wit
            })
        }

        Ok(witness)
    }

    /// Returns the stdout buffer.
    pub fn std_out(&self) -> &[u8] {
        self.std_out.buffer()
    }

    /// Returns the stderr buffer.
    pub fn std_err(&self) -> &[u8] {
        self.std_err.buffer()
    }
}

#[cfg(test)]
mod test {
    use alloy_primitives::keccak256;

    use crate::test_utils::{ClaimTestOracle, BASE_ADDR_END, END_ADDR};
    use crate::witness::STATE_WITNESS_SIZE;
    use crate::{load_elf, patch, StateWitnessHasher};
    use crate::{test_utils::StaticOracle, Address, InstrumentedState, Memory, State};
    use std::io::BufWriter;
    use std::{
        fs,
        io::{self, BufReader},
        path::PathBuf,
    };

    mod open_mips {
        use super::*;

        #[test]
        fn open_mips_tests() {
            let tests_path = PathBuf::from(std::env::current_dir().unwrap())
                .join("open_mips_tests")
                .join("test")
                .join("bin");
            let test_files = fs::read_dir(tests_path).unwrap();

            for f in test_files.into_iter() {
                if let Ok(f) = f {
                    let file_name = String::from(f.file_name().to_str().unwrap());

                    // Short circuit early for `exit_group.bin`
                    let exit_group = file_name == "exit_group.bin";

                    let program_mem = fs::read(f.path()).unwrap();

                    let mut state = {
                        let mut state = State::default();
                        state.pc = 0;
                        state.next_pc = 4;
                        state.memory = Memory::default();
                        state
                    };
                    state
                        .memory
                        .set_memory_range(0, BufReader::new(program_mem.as_slice()))
                        .unwrap();

                    // Set the return address ($ra) to jump into when the test completes.
                    state.registers[31] = END_ADDR;

                    let mut ins = InstrumentedState::new(
                        state,
                        StaticOracle::new(b"hello world".to_vec()),
                        io::stdout(),
                        io::stderr(),
                    );

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
                        let mut state = ins.state.memory;
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

    #[test]
    fn state_hash() {
        let cases = [
            (false, 0),
            (false, 1),
            (false, 2),
            (false, 3),
            (true, 0),
            (true, 1),
            (true, 2),
            (true, 3),
        ];

        for (exited, exit_code) in cases.into_iter() {
            let mut state = State {
                exited,
                exit_code,
                ..Default::default()
            };

            let actual_witness = state.encode_witness().unwrap();
            let actual_state_hash = actual_witness.state_hash();
            assert_eq!(actual_witness.len(), STATE_WITNESS_SIZE);

            let mut expected_witness = [0u8; STATE_WITNESS_SIZE];
            let mem_root = state.memory.merkle_root().unwrap();
            expected_witness[..32].copy_from_slice(mem_root.as_slice());
            expected_witness[32 * 2 + 4 * 6] = exit_code;
            expected_witness[32 * 2 + 4 * 6 + 1] = exited as u8;

            assert_eq!(actual_witness, expected_witness, "Incorrect witness");

            let mut expected_state_hash = keccak256(&expected_witness);
            expected_state_hash[0] = State::vm_status(exited, exit_code) as u8;
            assert_eq!(
                actual_state_hash, expected_state_hash,
                "Incorrect state hash"
            );
        }
    }

    #[test]
    fn test_hello() {
        let elf_bytes = include_bytes!("../../../../example/bin/hello.elf");
        let mut state = load_elf(elf_bytes).unwrap();
        patch::patch_go(elf_bytes, &mut state).unwrap();
        patch::patch_stack(&mut state).unwrap();

        let out = BufWriter::new(Vec::default());
        let err = BufWriter::new(Vec::default());
        let mut ins =
            InstrumentedState::new(state, StaticOracle::new(b"hello world".to_vec()), out, err);

        for _ in 0..400_000 {
            if ins.state.exited {
                break;
            }
            ins.step(false).unwrap();
        }

        assert!(ins.state.exited, "must exit");
        assert_eq!(ins.state.exit_code, 0, "must exit with 0");

        assert_eq!(
            String::from_utf8(ins.std_out.buffer().to_vec()).unwrap(),
            "hello world!\n"
        );
        assert_eq!(
            String::from_utf8(ins.std_err.buffer().to_vec()).unwrap(),
            ""
        );
    }

    #[test]
    fn test_claim() {
        let elf_bytes = include_bytes!("../../../../example/bin/claim.elf");
        let mut state = load_elf(elf_bytes).unwrap();
        patch::patch_go(elf_bytes, &mut state).unwrap();
        patch::patch_stack(&mut state).unwrap();

        let out = BufWriter::new(Vec::default());
        let err = BufWriter::new(Vec::default());
        let mut ins = InstrumentedState::new(state, ClaimTestOracle::default(), out, err);

        for _ in 0..2_000_000 {
            if ins.state.exited {
                break;
            }
            ins.step(false).unwrap();
        }

        assert!(ins.state.exited, "must exit");
        assert_eq!(ins.state.exit_code, 0, "must exit with 0");

        assert_eq!(
            String::from_utf8(ins.std_out.buffer().to_vec()).unwrap(),
            format!(
                "computing {} * {} + {}\nclaim {} is good!\n",
                ClaimTestOracle::S,
                ClaimTestOracle::A,
                ClaimTestOracle::B,
                ClaimTestOracle::S * ClaimTestOracle::A + ClaimTestOracle::B
            )
        );
        assert_eq!(
            String::from_utf8(ins.std_err.buffer().to_vec()).unwrap(),
            "started!"
        );
    }
}
