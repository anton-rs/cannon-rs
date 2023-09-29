//! Subcommands for the `cannon` binary

use clap::Subcommand;

mod run;
mod witness;

pub(crate) trait CannonSubcommandDispatcher {
    /// Dispatches the subcommand
    fn dispatch(&self);
}

/// The subcommands for the `cannon` binary
#[derive(Subcommand, Debug)]
pub(crate) enum CannonSubcommand {
    Run(run::RunArgs),
    Witness(witness::WitnessArgs),
}

impl CannonSubcommandDispatcher for CannonSubcommand {
    /// Dispatches the subcommand
    fn dispatch(&self) {
        match self {
            CannonSubcommand::Run(args) => args.dispatch(),
            CannonSubcommand::Witness(args) => args.dispatch(),
        }
    }
}
