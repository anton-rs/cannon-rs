//! The `witness` subcommand for the cannon binary

use clap::Args;

use super::CannonSubcommandDispatcher;

/// Command line arguments for `cannon witness`
#[derive(Args, Debug)]
#[command(author, version, about)]
pub(crate) struct WitnessArgs {
    /// The path to the input JSON state.
    #[arg(long)]
    input: String,

    /// The path to the output JSON state.
    #[arg(long)]
    output: String,
}

impl CannonSubcommandDispatcher for WitnessArgs {
    fn dispatch(&self) {}
}
