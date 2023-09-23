//! Testing utilities.

use crate::PreimageOracle;
use alloy_primitives::B256;
use preimage_oracle::{Keccak256Key, Key};

pub mod evm;

/// Used in tests to write the results to
pub const BASE_ADDR_END: u32 = 0xBF_FF_FF_F0;

/// Used as the return-address for tests
pub const END_ADDR: u32 = 0xA7_EF_00_D0;

pub struct StaticOracle {
    preimage_data: Vec<u8>,
}

impl StaticOracle {
    pub fn new(preimage_data: Vec<u8>) -> Self {
        Self { preimage_data }
    }
}

impl PreimageOracle for StaticOracle {
    fn hint(&mut self, _value: &[u8]) {
        // noop
    }

    fn get(&self, key: B256) -> anyhow::Result<&[u8]> {
        if key != (key as Keccak256Key).preimage_key() {
            anyhow::bail!("Invalid preimage ")
        }
        Ok(self.preimage_data.as_slice())
    }
}
