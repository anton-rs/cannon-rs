//! Testing utilities.

use crate::{utils::concat_fixed, PreimageOracle};
use alloy_primitives::{hex, keccak256, B256};
use once_cell::sync::Lazy;
use preimage_oracle::{Keccak256Key, Key, LocalIndexKey};
use revm::primitives::HashMap;

pub mod evm;

/// Used in tests to write the results to
pub const BASE_ADDR_END: u32 = 0xBF_FF_FF_F0;

/// Used as the return-address for tests
pub const END_ADDR: u32 = 0xA7_EF_00_D0;

#[derive(Default)]
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

pub struct ClaimTestOracle {
    images: HashMap<B256, Vec<u8>>,
}

impl ClaimTestOracle {
    const S: u64 = 1000;
    const A: u64 = 3;
    const B: u64 = 4;
    const DIFF: Lazy<[u8; 64]> = Lazy::new(|| {
        concat_fixed(
            keccak256(Self::A.to_be_bytes()).into(),
            keccak256(Self::B.to_be_bytes()).into(),
        )
    });
    const PRE_HASH: Lazy<B256> = Lazy::new(|| keccak256(Self::S.to_be_bytes()));
    const DIFF_HASH: Lazy<B256> = Lazy::new(|| keccak256(Self::DIFF.as_slice()));
}

impl Default for ClaimTestOracle {
    fn default() -> Self {
        let mut s = Self {
            images: HashMap::new(),
        };

        s.images
            .insert((0 as LocalIndexKey).preimage_key(), Self::PRE_HASH.to_vec());
        s.images.insert(
            (1 as LocalIndexKey).preimage_key(),
            Self::DIFF_HASH.to_vec(),
        );
        s.images.insert(
            (2 as LocalIndexKey).preimage_key(),
            (Self::S * Self::A + Self::B).to_be_bytes().to_vec(),
        );

        s
    }
}

impl PreimageOracle for ClaimTestOracle {
    fn hint(&mut self, value: &[u8]) {
        let s = String::from_utf8(value.to_vec()).unwrap();
        let parts: Vec<&str> = s.split(" ").collect();

        assert_eq!(parts.len(), 2);

        let part = hex::decode(parts[1]).unwrap();
        assert_eq!(part.len(), 32);
        let hash = B256::from_slice(&part);

        match parts[0] {
            "fetch-state" => {
                assert_eq!(
                    hash,
                    *Self::PRE_HASH,
                    "Expecting request for pre-state preimage"
                );

                self.images.insert(
                    (*Self::PRE_HASH as Keccak256Key).preimage_key(),
                    Self::S.to_be_bytes().to_vec(),
                );
            }
            "fetch-diff" => {
                assert_eq!(
                    hash,
                    *Self::DIFF_HASH,
                    "Expecting request for diff preimage"
                );
                self.images.insert(
                    (*Self::DIFF_HASH as Keccak256Key).preimage_key(),
                    Self::DIFF.to_vec(),
                );
                self.images.insert(
                    (keccak256(Self::A.to_be_bytes()) as Keccak256Key).preimage_key(),
                    Self::A.to_be_bytes().to_vec(),
                );
                self.images.insert(
                    (keccak256(Self::B.to_be_bytes()) as Keccak256Key).preimage_key(),
                    Self::B.to_be_bytes().to_vec(),
                );
            }
            _ => panic!("Unexpected hint: {}", parts[0]),
        }
    }

    fn get(&self, key: B256) -> anyhow::Result<&[u8]> {
        Ok(self
            .images
            .get(&key)
            .ok_or(anyhow::anyhow!("No image for key"))?)
    }
}
