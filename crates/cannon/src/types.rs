//! This module contains the types for the `cannon` interface.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Proof {
    step: u64,
    pre: [u8; 32],
    post: [u8; 32],
    state_data: Vec<u8>,
    proof_data: Vec<u8>,
    oracle_key: Option<Vec<u8>>,
    oracle_value: Option<Vec<u8>>,
    oracle_offset: Option<u32>,
    step_input: Vec<u8>,
    oracle_input: Vec<u8>,
}
