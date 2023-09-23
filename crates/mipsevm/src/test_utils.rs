//! Testing utilities.

use alloy_primitives::B256;
use preimage_oracle::{Keccak256Key, Key};

use crate::PreimageOracle;

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
