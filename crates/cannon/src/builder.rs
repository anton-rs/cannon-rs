//! The [KernelBuilder] struct is a helper for building a [Kernel] struct.

use crate::{gz, ChildWithFds, Kernel, ProcessPreimageOracle};
use anyhow::{anyhow, Result};
use cannon_mipsevm::{InstrumentedState, State};
use std::{
    fs,
    io::{self, Stderr, Stdout},
    path::PathBuf,
};

/// The [KernelBuilder] struct is a helper for building a [Kernel] struct.
#[derive(Default, Debug)]
pub struct KernelBuilder {
    /// The full command to run the preimage server
    preimage_server: String,
    /// The path to the input JSON state.
    input: String,
    /// The path to the output JSON state.
    output: Option<String>,
    /// The step to generate an output proof at.
    proof_at: Option<String>,
    /// Format for proof data output file names. Proof data is written to stdout
    /// if this is not specified.
    proof_format: Option<String>,
    /// The step pattern to generate state snapshots at.
    snapshot_at: Option<String>,
    /// Format for snapshot data output file names.
    snapshot_format: Option<String>,
    /// The instruction step to stop running at.
    stop_at: Option<String>,
    /// The pattern to print information at.
    info_at: Option<String>,
}

impl KernelBuilder {
    /// Builds the [Kernel] struct from the information contained within the [KernelBuilder].
    ///
    /// TODO(clabby): Make the i/o streams + the preimage oracle configurable.
    pub fn build(self) -> Result<Kernel<Stdout, Stderr, ProcessPreimageOracle>> {
        // Read the compressed state dump from the input file, decompress it, and deserialize it.
        let raw_state = fs::read(&self.input)?;
        let state: State = serde_json::from_slice(&gz::decompress_bytes(&raw_state)?)?;

        let (hint_cl_rw, hint_oracle_rw) = preimage_oracle::create_bidirectional_channel()?;
        let (pre_cl_rw, pre_oracle_rw) = preimage_oracle::create_bidirectional_channel()?;

        let server_io = [hint_oracle_rw, pre_oracle_rw];

        // TODO(clabby): Allow for the preimage server to be configurable.
        let cmd = self
            .preimage_server
            .split(' ')
            .map(String::from)
            .collect::<Vec<_>>();
        let (oracle, server_proc) = ProcessPreimageOracle::start(
            PathBuf::from(
                cmd.get(0)
                    .ok_or(anyhow!("Missing preimage server binary path"))?,
            ),
            &cmd[1..],
            (hint_cl_rw, pre_cl_rw),
            &server_io,
        )?;

        let server_proc = server_proc.map(|p| ChildWithFds {
            inner: p,
            fds: server_io,
        });

        // TODO(clabby): Allow for the stdout / stderr to be configurable.
        let instrumented = InstrumentedState::new(state, oracle, io::stdout(), io::stderr());

        Ok(Kernel::new(
            instrumented,
            server_proc,
            self.input,
            self.output,
            self.proof_at,
            self.proof_format,
            self.snapshot_at,
            self.snapshot_format,
            self.stop_at,
            self.info_at,
        ))
    }

    pub fn with_preimage_server(mut self, preimage_server: String) -> Self {
        self.preimage_server = preimage_server;
        self
    }

    pub fn with_input(mut self, input: String) -> Self {
        self.input = input;
        self
    }

    pub fn with_output(mut self, output: Option<String>) -> Self {
        self.output = output;
        self
    }

    pub fn with_proof_at(mut self, proof_at: Option<String>) -> Self {
        self.proof_at = proof_at;
        self
    }

    pub fn with_proof_format(mut self, proof_format: Option<String>) -> Self {
        self.proof_format = proof_format;
        self
    }

    pub fn with_snapshot_at(mut self, snapshot_at: Option<String>) -> Self {
        self.snapshot_at = snapshot_at;
        self
    }

    pub fn with_snapshot_format(mut self, snapshot_format: Option<String>) -> Self {
        self.snapshot_format = snapshot_format;
        self
    }

    pub fn with_stop_at(mut self, stop_at: Option<String>) -> Self {
        self.stop_at = stop_at;
        self
    }

    pub fn with_info_at(mut self, info_at: Option<String>) -> Self {
        self.info_at = info_at;
        self
    }
}
