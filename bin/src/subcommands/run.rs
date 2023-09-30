//! The `run` subcommand for the cannon binary

use super::CannonSubcommandDispatcher;
use anyhow::Result;
use async_trait::async_trait;
use cannon::KernelBuilder;
use clap::Args;

/// Command line arguments for `cannon run`
#[derive(Args, Debug)]
#[command(author, version, about)]
pub(crate) struct RunArgs {
    /// The preimage oracle command
    #[arg(long)]
    preimage_server: String,

    /// The path to the input JSON state.
    #[arg(long)]
    input: String,

    /// The path to the output JSON state.
    #[arg(long)]
    output: Option<String>,

    /// The step to generate an output proof at.
    #[arg(long)]
    proof_at: Option<String>,

    /// Format for proof data output file names. Proof data is written to stdout
    /// if this is not specified.
    #[arg(long, aliases = ["proof-fmt"])]
    proof_format: Option<String>,

    /// The step pattern to generate state snapshots at.
    #[arg(long)]
    snapshot_at: Option<String>,

    /// Format for snapshot data output file names.
    #[arg(long, aliases = ["snapshot-fmt"])]
    snapshot_format: Option<String>,

    /// The instruction step to stop running at.
    #[arg(long)]
    stop_at: Option<String>,

    /// The pattern to print information at.
    #[arg(long)]
    info_at: Option<String>,
}

#[async_trait]
impl CannonSubcommandDispatcher for RunArgs {
    async fn dispatch(self) -> Result<()> {
        let kernel = KernelBuilder::default()
            .with_preimage_server(self.preimage_server)
            .with_input(self.input)
            .with_output(self.output)
            .with_proof_at(self.proof_at)
            .with_proof_format(self.proof_format)
            .with_snapshot_at(self.snapshot_at)
            .with_snapshot_format(self.snapshot_format)
            .with_stop_at(self.stop_at)
            .with_info_at(self.info_at)
            .build()?;
        kernel.run()
    }
}
