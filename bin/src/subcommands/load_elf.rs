//! The `load-elf` subcommand for the cannon binary

use super::CannonSubcommandDispatcher;
use anyhow::Result;
use async_trait::async_trait;
use cannon::gz::compress_bytes;
use cannon_mipsevm::{load_elf, patch_go, patch_stack};
use clap::{builder::PossibleValue, Args, ValueEnum};
use std::{fs, path::PathBuf};

/// Command line arguments for `cannon load-elf`
#[derive(Args, Debug)]
#[command(author, version, about)]
pub(crate) struct LoadElfArgs {
    /// The path to the input 32-bit big-endian MIPS ELF file.
    #[arg(long)]
    path: PathBuf,

    /// The type of patch to perform on the ELF file.
    #[arg(long, value_enum)]
    patch_kind: PatchKind,

    /// The output path to write the JSON state to. State will be dumped to stdout if set to `-`.
    /// Not written if not provided.
    #[arg(long)]
    output: Option<String>,
}

#[derive(Clone, Debug)]
enum PatchKind {
    Go,
    Stack,
}

impl ValueEnum for PatchKind {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Go, Self::Stack]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::Go => PossibleValue::new("go"),
            Self::Stack => PossibleValue::new("stack"),
        })
    }
}

#[async_trait]
impl CannonSubcommandDispatcher for LoadElfArgs {
    async fn dispatch(self) -> Result<()> {
        tracing::info!(target: "cannon-cli::load-elf", "Loading ELF file @ {}", self.path.display());
        let elf_raw = fs::read(&self.path)?;
        let mut state = load_elf(&elf_raw)?;
        tracing::info!(target: "cannon-cli::load-elf", "Loaded ELF file and constructed the State");

        match self.patch_kind {
            PatchKind::Go => {
                tracing::info!(target: "cannon-cli::load-elf", "Patching the ELF file with patch type = Go...");
                patch_go(&elf_raw, &mut state)
            }
            PatchKind::Stack => {
                tracing::info!(target: "cannon-cli::load-elf", "Patching the ELF file with patch type = Stack...");
                patch_stack(&mut state)
            }
        }?;

        if let Some(ref path_str) = self.output {
            if path_str == "-" {
                println!("{}", serde_json::to_string(&state)?);
            } else {
                fs::write(path_str, compress_bytes(&serde_json::to_vec(&state)?)?)?;
            }
        }

        tracing::info!(target: "cannon-cli::load-elf", "Patched the ELF file and dumped the State successfully.");

        Ok(())
    }
}
