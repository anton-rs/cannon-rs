//! This module contains a wrapper around a [revm] inspector with an in-memory backend
//! that has the MIPS & PreimageOracle smart contracts deployed at deterministic addresses.

use crate::{StateWitness, StateWitnessHasher, StepWitness};
use anyhow::Result;
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{
        hex, AccountInfo, Address, Bytecode, Bytes, CreateScheme, Output, ResultAndState,
        TransactTo, TxEnv, B256, U256,
    },
    Database, EVM,
};

/// The address of the deployed MIPS VM on the in-memory EVM.
pub const MIPS_ADDR: [u8; 20] = hex!("000000000000000000000000000000000000C0DE");
/// The address of the deployed PreimageOracle on the in-memory EVM.
pub const PREIMAGE_ORACLE_ADDR: [u8; 20] = hex!("00000000000000000000000000000000424f4f4b");

/// The creation EVM bytecode of the MIPS contract. Does not include constructor arguments.
pub const MIPS_CREATION_CODE: &str = include_str!("../../bindings/mips_creation.bin");
/// The deployed EVM bytecode of the PreimageOracle contract.
pub const PREIMAGE_ORACLE_DEPLOYED_CODE: &str =
    include_str!("../../bindings/preimage_oracle_deployed.bin");

/// A wrapper around a [revm] inspector with an in-memory backend that has the MIPS & PreimageOracle
/// smart contracts deployed at deterministic addresses. This is used for differential testing the
/// implementation of the MIPS VM in this crate against the smart contract implementations.
pub struct MipsEVM<DB: Database> {
    pub inner: EVM<DB>,
}

impl Default for MipsEVM<CacheDB<EmptyDB>> {
    fn default() -> Self {
        Self::new()
    }
}

impl MipsEVM<CacheDB<EmptyDB>> {
    /// Creates a new MIPS EVM with an in-memory backend.
    pub fn new() -> Self {
        let mut evm = EVM::default();
        evm.database(CacheDB::default());

        Self { inner: evm }
    }

    /// Initializes the EVM with the MIPS contracts deployed.
    ///
    /// ### Returns
    /// - A [Result] indicating whether the initialization was successful.
    pub fn try_init(&mut self) -> Result<()> {
        let db = self.inner.db().ok_or(anyhow::anyhow!("Missing database"))?;

        // Fund the zero address.
        db.insert_account_info(
            Address::ZERO,
            AccountInfo {
                balance: U256::from(u128::MAX),
                nonce: 0,
                code_hash: B256::ZERO,
                code: None,
            },
        );

        // Deploy the PreimageOracle contract.
        self.deploy_contract(
            Address::from_slice(PREIMAGE_ORACLE_ADDR.as_slice()),
            Bytes::from(hex::decode(PREIMAGE_ORACLE_DEPLOYED_CODE)?),
        )?;

        // Deploy the MIPS contract prior to deploying it manually. This contract has an immutable
        // variable, so we let the creation code fill this in for us, and then deploy it to the
        // test address.
        let encoded_preimage_addr =
            Address::from_slice(PREIMAGE_ORACLE_ADDR.as_slice()).into_word();
        let mips_creation_heap = hex::decode(MIPS_CREATION_CODE)?
            .into_iter()
            .chain(encoded_preimage_addr)
            .collect::<Vec<_>>();
        self.fill_tx_env(
            TransactTo::Create(CreateScheme::Create),
            mips_creation_heap.into(),
        );
        if let Ok(ResultAndState {
            result:
                revm::primitives::ExecutionResult::Success {
                    reason: _,
                    gas_used: _,
                    gas_refunded: _,
                    logs: _,
                    output: Output::Create(code, _),
                },
            state: _,
        }) = self.inner.transact_ref()
        {
            // Deploy the MIPS contract manually.
            self.deploy_contract(Address::from_slice(MIPS_ADDR.as_slice()), code)
        } else {
            anyhow::bail!("Failed to deploy MIPS contract");
        }
    }

    /// Perform a single instruction step on the MIPS smart contract from the VM state encoded
    /// in the [StepWitness] passed.
    ///
    /// ### Takes
    /// - `witness`: The [StepWitness] containing the VM state to step.
    ///
    /// ### Returns
    /// - A [Result] containing the post-state hash of the MIPS VM or an error returned during
    /// execution.
    pub fn step(&mut self, witness: StepWitness) -> Result<StateWitness> {
        if witness.has_preimage() {
            crate::debug!(
                target: "mipsevm::evm",
                "Reading preimage key {:x} at offset {:?}",
                B256::from(witness.preimage_key.ok_or(anyhow::anyhow!("Missing preimage key"))?),
                witness.preimage_offset
            );

            let preimage_oracle_input =
                witness
                    .encode_preimage_oracle_input()
                    .ok_or(anyhow::anyhow!(
                        "Failed to ABI encode preimage oracle input."
                    ))?;
            self.fill_tx_env(
                TransactTo::Call(PREIMAGE_ORACLE_ADDR.into()),
                preimage_oracle_input,
            );
            self.inner.transact_commit().map_err(|_| {
                anyhow::anyhow!("Failed to commit preimage to PreimageOracle contract")
            })?;
        }

        crate::debug!(target: "mipsevm::evm", "Performing EVM step");

        let step_input = witness.encode_step_input();
        self.fill_tx_env(TransactTo::Call(MIPS_ADDR.into()), step_input);
        if let Ok(ResultAndState {
            result:
                revm::primitives::ExecutionResult::Success {
                    reason: _,
                    gas_used: _,
                    gas_refunded: _,
                    logs,
                    output: Output::Call(output),
                },
            state: _,
        }) = self.inner.transact_ref()
        {
            let output = B256::from_slice(&output);

            crate::debug!(target: "mipsevm::evm", "EVM step successful with resulting post-state hash: {:x}", output);

            if logs.len() != 1 {
                anyhow::bail!("Expected 1 log, got {}", logs.len());
            }

            let post_state: StateWitness = logs[0].data.to_vec().as_slice().try_into()?;

            if post_state.state_hash().as_slice() != output.as_slice() {
                anyhow::bail!(
                    "Post-state hash does not match state hash in log: {:x} != {:x}",
                    output,
                    B256::from(post_state.state_hash())
                );
            }

            Ok(post_state)
        } else {
            anyhow::bail!("Failed to step MIPS contract");
        }
    }

    /// Deploys a contract with the given code at the given address.
    ///
    /// ### Takes
    /// - `db`: The database backend of the MIPS EVM.
    /// - `addr`: The address to deploy the contract to.
    /// - `code`: The code of the contract to deploy.
    pub(crate) fn deploy_contract(&mut self, addr: Address, code: Bytes) -> Result<()> {
        let mut acc_info = AccountInfo {
            balance: U256::ZERO,
            nonce: 0,
            code_hash: B256::ZERO,
            code: Some(Bytecode::new_raw(code)),
        };
        let db = self.inner.db().ok_or(anyhow::anyhow!("Missing database"))?;
        db.insert_contract(&mut acc_info);
        db.insert_account_info(addr, acc_info);
        Ok(())
    }

    /// Fills the transaction environment with the given data.
    ///
    /// ### Takes
    /// - `transact_to`: The transaction type.
    /// - `data`: The calldata for the transaction.
    /// - `to`: The address of the contract to call.
    pub(crate) fn fill_tx_env(&mut self, transact_to: TransactTo, data: Bytes) {
        self.inner.env.tx = TxEnv {
            caller: Address::ZERO,
            gas_limit: u64::MAX,
            gas_price: U256::ZERO,
            gas_priority_fee: None,
            transact_to,
            value: U256::ZERO,
            data,
            chain_id: None,
            nonce: None,
            access_list: Vec::default(),
            blob_hashes: Vec::default(),
            max_fee_per_blob_gas: None,
        };
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        patch,
        test_utils::{ClaimTestOracle, StaticOracle, BASE_ADDR_END, END_ADDR},
        Address, InstrumentedState, Memory, State,
    };
    use revm::primitives::ExecutionResult;
    use std::{
        fs,
        io::{self, BufReader, BufWriter},
        path::PathBuf,
    };

    #[test]
    fn sanity_evm_execution() {
        const SAMPLE: [u8; 2180] = hex!("f8e0cb960000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e22306a30adb7e99858491484b0d6627fe00efea43ec78488033a797a499e22ad6000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007000e000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ad3228b676f7d3cd4284a5443f17f1962b36e491b30a40b2405849e597ba5fb5b4c11951957c6f8f642c4af61cd6b24640fec6dc7fc607ee8206a99e92410d3021ddb9a356815c3fac1026b6dec5df3124afbadb485c9ba5a3e3398a04b7ba85e58769b32a1beaf1ea27375a44095a0d1fb664ce2dd358e7fcbfb78c26a193440eb01ebfc9ed27500cd4dfc979272d1f0913cc9f66540d7e8005811109e1cf2d887c22bd8750d34016ac3c66b5ff102dacdd73f6b014e710b51e8022af9a1968ffd70157e48063fc33c97a050f7f640233bf646cc98d9524c6b92bcf3ab56f839867cc5f7f196b93bae1e27e6320742445d290f2263827498b54fec539f756afcefad4e508c098b9a7e1d8feb19955fb02ba9675585078710969d3440f5054e0f9dc3e7fe016e050eff260334f18a5d4fe391d82092319f5964f2e2eb7c1c3a5f8b13a49e282f609c317a833fb8d976d11517c571d1221a265d25af778ecf8923490c6ceeb450aecdc82e28293031d10c7d73bf85e57bf041a97360aa2c5d99cc1df82d9c4b87413eae2ef048f94b4d3554cea73d92b0f7af96e0271c691e2bb5c67add7c6caf302256adedf7ab114da0acfe870d449a3a489f781d659e8beccda7bce9f4e8618b6bd2f4132ce798cdc7a60e7e1460a7299e3c6342a579626d22733e50f526ec2fa19a22b31e8ed50f23cd1fdf94c9154ed3a7609a2f1ff981fe1d3b5c807b281e4683cc6d6315cf95b9ade8641defcb32372f1c126e398ef7a5a2dce0a8a7f68bb74560f8f71837c2c2ebbcbf7fffb42ae1896f13f7c7479a0b46a28b6f55540f89444f63de0378e3d121be09e06cc9ded1c20e65876d36aa0c65e9645644786b620e2dd2ad648ddfcbf4a7e5b1a3a4ecfe7f64667a3f0b7e2f4418588ed35a2458cffeb39b93d26f18d2ab13bdce6aee58e7b99359ec2dfd95a9c16dc00d6ef18b7933a6f8dc65ccb55667138776f7dea101070dc8796e3774df84f40ae0c8229d0d6069e5c8f39a7c299677a09d367fc7b05e3bc380ee652cdc72595f74c7b1043d0e1ffbab734648c838dfb0527d971b602bc216c9619ef0abf5ac974a1ed57f4050aa510dd9c74f508277b39d7973bb2dfccc5eeb0618db8cd74046ff337f0a7bf2c8e03e10f642c1886798d71806ab1e888d9e5ee87d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

        let mut mips_evm = MipsEVM::new();
        mips_evm.try_init().unwrap();

        mips_evm.fill_tx_env(
            TransactTo::Call(MIPS_ADDR.into()),
            Bytes::from(SAMPLE.to_vec()),
        );

        let ResultAndState { result, state: _ } = mips_evm.inner.transact_ref().unwrap();

        assert!(result.is_success());
        let ExecutionResult::Success {
            reason: _,
            gas_used: _,
            gas_refunded: _,
            logs: _,
            output: Output::Call(output),
        } = result
        else {
            panic!("Expected success, got {:?}", result);
        };

        assert_eq!(
            output,
            Bytes::from_static(&hex!(
                "03720be420feea4ae4f803f0f630004f8bd2b0256171dd26043e48bf524da332"
            ))
        );
    }

    #[test]
    fn evm() {
        let mut mips_evm = MipsEVM::new();
        mips_evm.try_init().unwrap();

        let tests_path = PathBuf::from(std::env::current_dir().unwrap())
            .join("open_mips_tests")
            .join("test")
            .join("bin");
        let test_files = fs::read_dir(tests_path).unwrap();

        for f in test_files.into_iter() {
            if let Ok(f) = f {
                let file_name = String::from(f.file_name().to_str().unwrap());
                println!(" -> Running test: {file_name}");

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

                let mut instrumented = InstrumentedState::new(
                    state,
                    StaticOracle::new(b"hello world".to_vec()),
                    io::stdout(),
                    io::stderr(),
                );

                for _ in 0..1000 {
                    if instrumented.state.pc == END_ADDR {
                        break;
                    }
                    if exit_group && instrumented.state.exited {
                        break;
                    }

                    let instruction = instrumented
                        .state
                        .memory
                        .get_memory(instrumented.state.pc as Address)
                        .unwrap();
                    println!(
                        "{}",
                        format!(
                            "step: {} pc: 0x{:08x} insn: 0x{:08x}",
                            instrumented.state.step, instrumented.state.pc, instruction
                        )
                    );

                    let step_witness = instrumented.step(true).unwrap().unwrap();

                    // Verify that the post state matches
                    let evm_post = mips_evm.step(step_witness).unwrap();
                    let rust_post = instrumented.state.encode_witness().unwrap();

                    assert_eq!(evm_post, rust_post);
                }

                if exit_group {
                    assert_ne!(END_ADDR, instrumented.state.pc, "must not reach end");
                    assert!(instrumented.state.exited, "must exit");
                    assert_eq!(1, instrumented.state.exit_code, "must exit with 1");
                } else {
                    assert_eq!(END_ADDR, instrumented.state.pc, "must reach end");
                    let mut state = instrumented.state.memory;
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

    #[test]
    fn evm_single_step() {
        let mut mips_evm = MipsEVM::new();
        mips_evm.try_init().unwrap();

        let cases = [
            ("j MSB set target", 0, 4, 0x0A_00_00_02),
            (
                "j non-zero PC region",
                0x10_00_00_00,
                0x10_00_00_04,
                0x08_00_00_02,
            ),
            ("jal MSB set target", 0, 4, 0x0E_00_00_02),
            (
                "jal non-zero PC region",
                0x10_00_00_00,
                0x10_00_00_04,
                0x0C_00_00_02,
            ),
        ];

        for (name, pc, next_pc, instruction) in cases {
            println!(" -> Running test: {name}");

            let mut state = State::default();
            state.pc = pc;
            state.next_pc = next_pc;
            state.memory.set_memory(pc, instruction).unwrap();

            let mut instrumented = InstrumentedState::new(
                state,
                StaticOracle::new(b"hello world".to_vec()),
                io::stdout(),
                io::stderr(),
            );
            let step_witness = instrumented.step(true).unwrap().unwrap();

            let evm_post = mips_evm.step(step_witness).unwrap();
            let rust_post = instrumented.state.encode_witness().unwrap();

            assert_eq!(evm_post, rust_post);
        }
    }

    #[test]
    fn evm_fault() {
        let mut mips_evm = MipsEVM::new();
        mips_evm.try_init().unwrap();

        let cases = [
            ("illegal instruction", 0, 0xFF_FF_FF_FFu32),
            ("branch in delay slot", 8, 0x11_02_00_03),
            ("jump in delay slot", 8, 0x0c_00_00_0c),
        ];

        for (name, next_pc, instruction) in cases {
            println!(" -> Running test: {name}");

            let mut state = State {
                next_pc: next_pc as Address,
                ..Default::default()
            };
            state.memory.set_memory(0, instruction).unwrap();

            // Set the return address ($ra) to jump to when the test completes.
            state.registers[31] = END_ADDR;

            let mut instrumented = InstrumentedState::new(
                state,
                StaticOracle::new(b"hello world".to_vec()),
                io::stdout(),
                io::stderr(),
            );
            assert!(instrumented.step(true).is_err());

            let mut initial_state = State {
                next_pc: next_pc as Address,
                memory: instrumented.state.memory.clone(),
                ..Default::default()
            };
            let instruction_proof = initial_state.memory.merkle_proof(0).unwrap();
            let step_witness = StepWitness {
                state: initial_state.encode_witness().unwrap(),
                mem_proof: instruction_proof.to_vec(),
                preimage_key: None,
                preimage_value: None,
                preimage_offset: None,
            };
            assert!(mips_evm.step(step_witness).is_err());
        }
    }

    #[test]
    fn test_hello_evm() {
        let mut mips_evm = MipsEVM::new();
        mips_evm.try_init().unwrap();

        let elf_bytes = include_bytes!("../../../../example/bin/hello.elf");
        let mut state = patch::load_elf(elf_bytes).unwrap();
        patch::patch_go(elf_bytes, &mut state).unwrap();
        patch::patch_stack(&mut state).unwrap();

        let mut instrumented =
            InstrumentedState::new(state, StaticOracle::default(), io::stdout(), io::stderr());

        for i in 0..400_000 {
            if instrumented.state.exited {
                break;
            }

            if i % 1000 == 0 {
                let instruction = instrumented
                    .state
                    .memory
                    .get_memory(instrumented.state.pc as Address)
                    .unwrap();
                println!(
                    "step: {} pc: 0x{:08x} instruction: {:08x}",
                    instrumented.state.step, instrumented.state.pc, instruction
                );
            }

            let step_witness = instrumented.step(true).unwrap().unwrap();

            let evm_post = mips_evm.step(step_witness).unwrap();
            let rust_post = instrumented.state.encode_witness().unwrap();
            assert_eq!(evm_post, rust_post);
        }

        assert!(instrumented.state.exited, "Must complete program");
        assert_eq!(instrumented.state.exit_code, 0, "Must exit with 0");
    }

    #[test]
    fn test_claim_evm() {
        let mut mips_evm = MipsEVM::new();
        mips_evm.try_init().unwrap();

        let elf_bytes = include_bytes!("../../../../example/bin/claim.elf");
        let mut state = patch::load_elf(elf_bytes).unwrap();
        patch::patch_go(elf_bytes, &mut state).unwrap();
        patch::patch_stack(&mut state).unwrap();

        let out_buf = BufWriter::new(Vec::default());
        let err_buf = BufWriter::new(Vec::default());

        let mut instrumented =
            InstrumentedState::new(state, ClaimTestOracle::default(), out_buf, err_buf);

        for i in 0..2_000_000 {
            if instrumented.state.exited {
                break;
            }

            if i % 1000 == 0 {
                let instruction = instrumented
                    .state
                    .memory
                    .get_memory(instrumented.state.pc as Address)
                    .unwrap();
                println!(
                    "step: {} pc: 0x{:08x} instruction: {:08x}",
                    instrumented.state.step, instrumented.state.pc, instruction
                );
            }

            let step_witness = instrumented.step(true).unwrap().unwrap();

            let evm_post = mips_evm.step(step_witness).unwrap();
            let rust_post = instrumented.state.encode_witness().unwrap();
            assert_eq!(evm_post, rust_post);
        }

        assert!(instrumented.state.exited, "Must complete program");
        assert_eq!(instrumented.state.exit_code, 0, "Must exit with 0");

        assert_eq!(
            String::from_utf8(instrumented.std_out.buffer().to_vec()).unwrap(),
            format!(
                "computing {} * {} + {}\nclaim {} is good!\n",
                ClaimTestOracle::S,
                ClaimTestOracle::A,
                ClaimTestOracle::B,
                ClaimTestOracle::S * ClaimTestOracle::A + ClaimTestOracle::B
            )
        );
        assert_eq!(
            String::from_utf8(instrumented.std_err.buffer().to_vec()).unwrap(),
            "started!"
        );
    }
}
