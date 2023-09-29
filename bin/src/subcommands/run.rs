//! The `run` subcommand for the cannon binary

use clap::Args;

use super::CannonSubcommandDispatcher;

/// Command line arguments for `cannon run`
#[derive(Args, Debug)]
#[command(author, version, about)]
pub(crate) struct RunArgs {
    /// The path to the input JSON state.
    #[arg(long)]
    input: String,

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

impl CannonSubcommandDispatcher for RunArgs {
    fn dispatch(&self) {
        todo!()
    }
}
