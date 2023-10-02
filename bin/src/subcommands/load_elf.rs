//! The `load-elf` subcommand for the cannon binary

use super::CannonSubcommandDispatcher;
use alloy_primitives::B256;
use anyhow::Result;
use cannon::gz::compress_bytes;
use cannon_mipsevm::{load_elf, patch_go, patch_stack, StateWitnessHasher};
use clap::Args;
use std::{fmt::Display, fs, path::PathBuf, str::FromStr};

/// Command line arguments for `cannon load-elf`
#[derive(Args, Debug)]
#[command(author, version, about)]
pub(crate) struct LoadElfArgs {
    /// The path to the input 32-bit big-endian MIPS ELF file.
    #[arg(long)]
    path: PathBuf,

    /// The type of patch to perform on the ELF file.
    #[arg(long, default_values = ["go", "stack"])]
    patch_kind: Vec<PatchKind>,

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

impl FromStr for PatchKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "go" => Ok(PatchKind::Go),
            "stack" => Ok(PatchKind::Stack),
            _ => Err(anyhow::anyhow!("Invalid patch kind: {}", s)),
        }
    }
}

impl Display for PatchKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchKind::Go => write!(f, "Go"),
            PatchKind::Stack => write!(f, "Stack"),
        }
    }
}

impl CannonSubcommandDispatcher for LoadElfArgs {
    fn dispatch(self) -> Result<()> {
        tracing::info!(target: "cannon-cli::load-elf", "Loading ELF file @ {}", self.path.display());
        let elf_raw = fs::read(&self.path)?;
        let mut state = load_elf(&elf_raw)?;
        tracing::info!(target: "cannon-cli::load-elf", "Loaded ELF file and constructed the State");

        for p in self.patch_kind {
            tracing::info!(target: "cannon-cli::load-elf", "Patching the ELF file with patch type = {p}...");
            match p {
                PatchKind::Go => patch_go(&elf_raw, &mut state),
                PatchKind::Stack => patch_stack(&mut state),
            }?;
        }

        if let Some(ref path_str) = self.output {
            if path_str == "-" {
                println!("{}", serde_json::to_string(&state)?);
            } else {
                fs::write(path_str, compress_bytes(&serde_json::to_vec(&state)?)?)?;
            }
        }

        tracing::info!(target: "cannon-cli::load-elf", "Patched the ELF file and dumped the State successfully. state hash: {} mem size: {} pages: {}", B256::from(state.encode_witness()?.state_hash()), state.memory.usage(), state.memory.page_count());

        Ok(())
    }
}
