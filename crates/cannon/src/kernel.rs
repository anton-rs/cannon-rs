//! This module contains the [Kernel] struct and its associated methods.

/// The [Kernel] struct contains the configuration for a Cannon kernel as well as
/// the [PreimageOracle] and [InstrumentedState] instances that form it.
#[allow(dead_code)]
pub struct Kernel {
    // ins_state: InstrumentedState,
    // preimage_oracle: PreimageOracle,
    /// The path to the input JSON state.
    input: String,
    /// The path to the output JSON state.
    output: Option<String>,
    /// The step to generate an output proof at.
    proof_at: Option<u64>,
    /// Format for proof data output file names. Proof data is written to stdout
    /// if this is not specified.
    proof_format: Option<String>,
    /// The step pattern to generate state snapshots at.
    snapshot_at: Option<String>,
    /// Format for snapshot data output file names.
    snapshot_format: Option<String>,
    /// The instruction step to stop running at.
    stop_at: Option<u64>,
    /// The pattern to print information at.
    info_at: Option<String>,
    /// An L1 RPC endpoint
    l1_endpoint: String,
    /// An L2 RPC endpoint
    l2_endpoint: String,
}

impl Kernel {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        input: String,
        output: Option<String>,
        proof_at: Option<u64>,
        proof_format: Option<String>,
        snapshot_at: Option<String>,
        snapshot_format: Option<String>,
        stop_at: Option<u64>,
        info_at: Option<String>,
        l1_endpoint: String,
        l2_endpoint: String,
    ) -> Self {
        Self {
            input,
            output,
            proof_at,
            proof_format,
            snapshot_at,
            snapshot_format,
            stop_at,
            info_at,
            l1_endpoint,
            l2_endpoint,
        }
    }
}
