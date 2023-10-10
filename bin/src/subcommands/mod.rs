//! Subcommands for the `cannon` binary

use anyhow::Result;
use clap::Subcommand;

mod load_elf;
mod run;
mod witness;

pub(crate) trait CannonSubcommandDispatcher {
    /// Dispatches the subcommand
    fn dispatch(self) -> Result<()>;
}

/// The subcommands for the `cannon` binary
#[derive(Subcommand, Debug)]
pub(crate) enum CannonSubcommand {
    Run(run::RunArgs),
    Witness(witness::WitnessArgs),
    LoadElf(load_elf::LoadElfArgs),
}

impl CannonSubcommandDispatcher for CannonSubcommand {
    fn dispatch(self) -> Result<()> {
        match self {
            CannonSubcommand::Run(args) => args.dispatch(),
            CannonSubcommand::Witness(args) => args.dispatch(),
            CannonSubcommand::LoadElf(args) => args.dispatch(),
        }
    }
}
