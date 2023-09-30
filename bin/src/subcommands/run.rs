//! The `run` subcommand for the cannon binary

use std::{fs, io, path::PathBuf};

use super::CannonSubcommandDispatcher;
use anyhow::Result;
use async_trait::async_trait;
use cannon::{compressor, ProcessPreimageOracle};
use cannon_mipsevm::{InstrumentedState, State};
use clap::Args;

/// Command line arguments for `cannon run`
#[derive(Args, Debug)]
#[command(author, version, about)]
pub(crate) struct RunArgs {
    /// The path to the input JSON state.
    #[arg(long)]
    input: String,

    /// The preimage oracle command
    #[arg(long, short)]
    preimage_oracle: String,

    /// The path to the output JSON state.
    #[arg(long)]
    output: Option<String>,

    /// The step to generate an output proof at.
    #[arg(long, short)]
    proof_at: Option<u64>,

    /// Format for proof data output file names. Proof data is written to stdout
    /// if this is not specified.
    #[arg(long)]
    proof_format: Option<String>,

    /// The step pattern to generate state snapshots at.
    #[arg(long, short)]
    snapshot_at: Option<String>,

    /// Format for snapshot data output file names.
    #[arg(long)]
    snapshot_format: Option<String>,

    /// The instruction step to stop running at.
    #[arg(long)]
    stop_at: Option<u64>,

    /// The pattern to print information at.
    #[arg(long, short)]
    info_at: Option<String>,

    /// An L1 RPC endpoint
    #[arg(long, aliases = ["le"])]
    l1_endpoint: String,

    /// An L2 RPC endpoint
    #[arg(long, aliases = ["la"])]
    l2_endpoint: String,
}

#[async_trait]
impl CannonSubcommandDispatcher for RunArgs {
    async fn dispatch(&self) -> Result<()> {
        let raw_state = fs::read(&self.input)?;
        let state: State = serde_json::from_slice(&compressor::decompress_bytes(&raw_state)?)?;

        let cmd = self
            .preimage_oracle
            .split(' ')
            .map(String::from)
            .collect::<Vec<_>>();
        let oracle = ProcessPreimageOracle::new(
            PathBuf::from(
                cmd.get(0)
                    .ok_or(anyhow::anyhow!("Missing preimage server binary path"))?,
            ),
            &cmd[1..],
        );

        let _instrumented = InstrumentedState::new(state, oracle, io::stdout(), io::stderr());

        todo!()
    }
}
