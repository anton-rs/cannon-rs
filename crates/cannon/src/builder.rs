use crate::Kernel;
use std::path::PathBuf;

/// The [KernelBuilder] struct is a helper for building a [Kernel] struct.
pub struct KernelBuilder {
    /// The path to the preimage oracle server.
    preimage_server: PathBuf,
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

impl KernelBuilder {
    /// Builds the [Kernel] struct from the information contained within the [KernelBuilder].
    pub fn build(self) -> Kernel {
        Kernel::new(
            self.input,
            self.output,
            self.proof_at,
            self.proof_format,
            self.snapshot_at,
            self.snapshot_format,
            self.stop_at,
            self.info_at,
            self.l1_endpoint,
            self.l2_endpoint,
        )
    }

    pub fn with_input(mut self, input: String) -> Self {
        self.input = input;
        self
    }

    pub fn with_output(mut self, output: String) -> Self {
        self.output = Some(output);
        self
    }

    pub fn with_proof_at(mut self, proof_at: u64) -> Self {
        self.proof_at = Some(proof_at);
        self
    }

    pub fn with_proof_format(mut self, proof_format: String) -> Self {
        self.proof_format = Some(proof_format);
        self
    }

    pub fn with_snapshot_at(mut self, snapshot_at: String) -> Self {
        self.snapshot_at = Some(snapshot_at);
        self
    }

    pub fn with_snapshot_format(mut self, snapshot_format: String) -> Self {
        self.snapshot_format = Some(snapshot_format);
        self
    }

    pub fn with_stop_at(mut self, stop_at: u64) -> Self {
        self.stop_at = Some(stop_at);
        self
    }

    pub fn with_info_at(mut self, info_at: String) -> Self {
        self.info_at = Some(info_at);
        self
    }

    pub fn with_l1_endpoint(mut self, l1_endpoint: String) -> Self {
        self.l1_endpoint = l1_endpoint;
        self
    }

    pub fn with_l2_endpoint(mut self, l2_endpoint: String) -> Self {
        self.l2_endpoint = l2_endpoint;
        self
    }
}
