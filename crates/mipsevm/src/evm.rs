//! This module contains a wrapper around a [revm] inspector with an in-memory backend
//! that has the MIPS & PreimageOracle smart contracts deployed at deterministic addresses.

use alloy_primitives::{hex, Address, U256};
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{
        AccountInfo, Bytecode, Bytes, CreateScheme, Output, ResultAndState, TransactTo, TxEnv,
        B160, B256,
    },
    Database, EVM,
};

pub const MIPS_ADDR: [u8; 20] = hex!("000000000000000000000000000000000000C0DE");
pub const PREIMAGE_ORACLE_ADDR: [u8; 20] = hex!("0000000000000000000000000000000000000420");

pub const MIPS_CREATION_CODE: &str = include_str!("../bindings/mips_creation.bin");
pub const PREIMAGE_ORACLE_DEPLOYED_CODE: &str =
    include_str!("../bindings/preimage_oracle_deployed.bin");

pub struct MipsEVM<DB: Database> {
    pub evm: EVM<DB>,
}

impl MipsEVM<CacheDB<EmptyDB>> {
    pub fn new() -> Self {
        let mut evm = EVM::default();
        evm.database(CacheDB::default());

        let Some(db) = evm.db() else {
            panic!("Database needs to be set");
        };

        db.insert_account_info(
            B160::zero(),
            AccountInfo {
                balance: U256::from(u128::MAX),
                nonce: 0,
                code_hash: B256::zero(),
                code: None,
            },
        );

        #[inline(always)]
        fn deploy_contract(db: &mut CacheDB<EmptyDB>, addr: B160, code: Bytes) {
            let mut acc_info = AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: B256::zero(),
                code: Some(Bytecode::new_raw(code)),
            };
            db.insert_contract(&mut acc_info);
            db.insert_account_info(addr, acc_info);
        }

        deploy_contract(
            db,
            B160::from_slice(PREIMAGE_ORACLE_ADDR.as_slice()),
            Bytes::from(hex::decode(PREIMAGE_ORACLE_DEPLOYED_CODE).unwrap()),
        );

        // Deploy the MIPS contract prior to setting it manually. This contract has an immutable
        // variable, so we let the creation code fill this in for us.
        let encoded_preimage_addr = Address::from_slice(MIPS_ADDR.as_slice()).into_word();
        let mips_creation_heap = hex::decode(MIPS_CREATION_CODE)
            .unwrap()
            .into_iter()
            .chain(encoded_preimage_addr)
            .collect::<Vec<_>>();
        evm.env.tx = TxEnv {
            caller: 0.into(),
            gas_limit: 0,
            gas_price: U256::ZERO,
            gas_priority_fee: None,
            transact_to: TransactTo::Create(CreateScheme::Create),
            value: U256::ZERO,
            data: mips_creation_heap.into(),
            chain_id: None,
            nonce: None,
            access_list: Vec::default(),
        };
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
        }) = evm.transact_ref()
        {
            // Deploy the MIPS contract manually.
            deploy_contract(
                evm.db().unwrap(),
                B160::from_slice(MIPS_ADDR.as_slice()),
                code,
            )
        } else {
            panic!("Failed to deploy MIPS contract");
        }

        Self { evm }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn evm() {
        let mut _mevm = MipsEVM::new();
        let acc = _mevm.evm.db().unwrap().load_account(MIPS_ADDR.into());
        dbg!(acc.unwrap());
    }
}
