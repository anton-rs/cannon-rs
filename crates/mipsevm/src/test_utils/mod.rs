//! Testing utilities.

use crate::{utils::concat_fixed, utils::keccak256, PreimageOracle};
use alloy_primitives::hex;
use anyhow::Result;
use preimage_oracle::{Hint, Keccak256Key, Key, LocalIndexKey};
use rustc_hash::FxHashMap;

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
    fn hint(&mut self, _value: impl Hint) -> Result<()> {
        // noop
        Ok(())
    }

    fn get(&mut self, key: [u8; 32]) -> anyhow::Result<Vec<u8>> {
        if key != (key as Keccak256Key).preimage_key() {
            anyhow::bail!("Invalid preimage ")
        }
        Ok(self.preimage_data.clone())
    }
}

pub struct ClaimTestOracle {
    images: FxHashMap<[u8; 32], Vec<u8>>,
}

impl ClaimTestOracle {
    pub(crate) const S: u64 = 1000;
    pub(crate) const A: u64 = 3;
    pub(crate) const B: u64 = 4;

    #[inline(always)]
    pub fn diff() -> [u8; 64] {
        concat_fixed(
            keccak256(Self::A.to_be_bytes()).into(),
            keccak256(Self::B.to_be_bytes()).into(),
        )
    }

    #[inline(always)]
    pub fn pre_hash() -> [u8; 32] {
        *keccak256(Self::S.to_be_bytes())
    }

    #[inline(always)]
    pub fn diff_hash() -> [u8; 32] {
        *keccak256(Self::diff().as_slice())
    }
}

impl Default for ClaimTestOracle {
    fn default() -> Self {
        let mut s = Self {
            images: Default::default(),
        };

        s.images.insert(
            (0 as LocalIndexKey).preimage_key(),
            Self::pre_hash().to_vec(),
        );
        s.images.insert(
            (1 as LocalIndexKey).preimage_key(),
            Self::diff_hash().to_vec(),
        );
        s.images.insert(
            (2 as LocalIndexKey).preimage_key(),
            (Self::S * Self::A + Self::B).to_be_bytes().to_vec(),
        );

        s
    }
}

impl PreimageOracle for ClaimTestOracle {
    fn hint(&mut self, value: impl Hint) -> Result<()> {
        let s = String::from_utf8(value.hint().to_vec()).unwrap();
        let parts: Vec<&str> = s.split(' ').collect();

        assert_eq!(parts.len(), 2);

        let part = hex::decode(parts[1]).unwrap();
        assert_eq!(part.len(), 32);
        let hash: [u8; 32] = part.try_into().unwrap();

        match parts[0] {
            "fetch-state" => {
                assert_eq!(
                    hash,
                    Self::pre_hash(),
                    "Expecting request for pre-state preimage"
                );

                self.images.insert(
                    (Self::pre_hash() as Keccak256Key).preimage_key(),
                    Self::S.to_be_bytes().to_vec(),
                );
            }
            "fetch-diff" => {
                assert_eq!(
                    hash,
                    Self::diff_hash(),
                    "Expecting request for diff preimage"
                );
                self.images.insert(
                    (Self::diff_hash() as Keccak256Key).preimage_key(),
                    Self::diff().to_vec(),
                );
                self.images.insert(
                    (*keccak256(Self::A.to_be_bytes()) as Keccak256Key).preimage_key(),
                    Self::A.to_be_bytes().to_vec(),
                );
                self.images.insert(
                    (*keccak256(Self::B.to_be_bytes()) as Keccak256Key).preimage_key(),
                    Self::B.to_be_bytes().to_vec(),
                );
            }
            _ => panic!("Unexpected hint: {}", parts[0]),
        }

        Ok(())
    }

    fn get(&mut self, key: [u8; 32]) -> anyhow::Result<Vec<u8>> {
        Ok(self
            .images
            .get(&key)
            .ok_or(anyhow::anyhow!("No image for key"))?
            .to_vec())
    }
}
