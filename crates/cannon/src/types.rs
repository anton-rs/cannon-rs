//! This module contains the types for the `cannon` interface.

use cannon_mipsevm::StateWitness;
use preimage_oracle::ReadWritePair;
use serde::{Deserialize, Serialize};
use std::process::Child;

/// The [Proof] struct contains the data for a Cannon proof at a given instruction.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proof {
    pub step: u64,
    pub pre: [u8; 32],
    pub post: [u8; 32],
    #[serde(with = "cannon_mipsevm::ser::state_witness_hex")]
    pub state_data: StateWitness,
    pub proof_data: Vec<u8>,
    pub step_input: Vec<u8>,
    pub oracle_key: Option<Vec<u8>>,
    pub oracle_value: Option<Vec<u8>>,
    pub oracle_offset: Option<u32>,
    pub oracle_input: Option<Vec<u8>>,
}

/// A [Child] process that was given file descriptors. This struct couples
/// the two together so that when the [Child] is dropped, the file descriptors
/// are as well, preventing a resource leak.
pub struct ChildWithFds {
    pub inner: Child,
    pub fds: [ReadWritePair; 2],
}
