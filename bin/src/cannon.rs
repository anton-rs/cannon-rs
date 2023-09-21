use anyhow::{anyhow, Result};
use clap::{ArgAction, Parser};
use tracing::Level;

/// Command line arguments for `cannon run`
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Verbosity level (0-4)
    #[arg(long, short, action = ArgAction::Count, default_value = "2")]
    v: u8,

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

fn main() -> Result<()> {
    // Parse the command arguments
    let Args {
        v,
        l1_endpoint: _,
        l2_endpoint: _,
        input: _,
        output: _,
        proof_at: _,
        proof_format: _,
        snapshot_at: _,
        snapshot_format: _,
        stop_at: _,
        info_at: _,
    } = Args::parse();

    // Initialize the tracing subscriber
    init_tracing_subscriber(v)?;

    tracing::info!(target: "cannon-cli", "TODO");

    Ok(())
}

/// Initializes the tracing subscriber
///
/// # Arguments
/// * `verbosity_level` - The verbosity level (0-4)
///
/// # Returns
/// * `Result<()>` - Ok if successful, Err otherwise.
fn init_tracing_subscriber(verbosity_level: u8) -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(match verbosity_level {
            0 => Level::ERROR,
            1 => Level::WARN,
            2 => Level::INFO,
            3 => Level::DEBUG,
            _ => Level::TRACE,
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber).map_err(|e| anyhow!(e))
}
