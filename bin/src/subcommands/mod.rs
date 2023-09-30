//! Subcommands for the `cannon` binary

use anyhow::Result;
use async_trait::async_trait;
use clap::Subcommand;

mod load_elf;
mod run;
mod witness;

#[async_trait]
pub(crate) trait CannonSubcommandDispatcher {
    /// Dispatches the subcommand
    async fn dispatch(self) -> Result<()>;
}

/// The subcommands for the `cannon` binary
#[derive(Subcommand, Debug)]
pub(crate) enum CannonSubcommand {
    Run(run::RunArgs),
    Witness(witness::WitnessArgs),
    LoadElf(load_elf::LoadElfArgs),
}

#[async_trait]
impl CannonSubcommandDispatcher for CannonSubcommand {
    async fn dispatch(self) -> Result<()> {
        match self {
            CannonSubcommand::Run(args) => args.dispatch().await,
            CannonSubcommand::Witness(args) => args.dispatch().await,
            CannonSubcommand::LoadElf(args) => args.dispatch().await,
        }
    }
}
