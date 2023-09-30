//! This module contains the [Kernel] struct and its associated methods.

use crate::{gz::compress_bytes, types::Proof};
use anyhow::{anyhow, Result};
use cannon_mipsevm::{InstrumentedState, PreimageOracle, StateWitnessHasher};
use std::{fs, io::Write};

#[cfg(feature = "tracing")]
use std::time::Instant;

/// The [Kernel] struct contains the configuration for a Cannon kernel as well as
/// the [PreimageOracle] and [InstrumentedState] instances that form it.
#[allow(dead_code)]
pub struct Kernel<O: Write, E: Write, P: PreimageOracle> {
    /// The instrumented state that the kernel will run.
    ins_state: InstrumentedState<O, E, P>,
    /// The path to the input JSON state.
    input: String,
    /// The path to the output JSON state.
    output: Option<String>,
    /// The step to generate an output proof at.
    proof_at: Option<String>,
    /// Format for proof data output file names. Proof data is written to stdout
    /// if this is not specified.
    proof_format: Option<String>,
    /// The step pattern to generate state snapshots at.
    snapshot_at: Option<String>,
    /// Format for snapshot data output file names.
    snapshot_format: Option<String>,
    /// The instruction step to stop running at.
    stop_at: Option<String>,
    /// The pattern to print information at.
    info_at: Option<String>,
}

impl<O, E, P> Kernel<O, E, P>
where
    O: Write,
    E: Write,
    P: PreimageOracle,
{
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        ins_state: InstrumentedState<O, E, P>,
        input: String,
        output: Option<String>,
        proof_at: Option<String>,
        proof_format: Option<String>,
        snapshot_at: Option<String>,
        snapshot_format: Option<String>,
        stop_at: Option<String>,
        info_at: Option<String>,
    ) -> Self {
        Self {
            ins_state,
            input,
            output,
            proof_at,
            proof_format,
            snapshot_at,
            snapshot_format,
            stop_at,
            info_at,
        }
    }

    pub fn run(mut self) -> Result<()> {
        let stop_at = create_matcher(self.stop_at.as_ref())?;
        let proof_at = create_matcher(self.proof_at.as_ref())?;
        let snapshot_at = create_matcher(self.snapshot_at.as_ref())?;

        #[cfg(feature = "tracing")]
        let (info_at, start_step, start) = (
            create_matcher(self.info_at.as_ref())?,
            self.ins_state.state.step,
            Instant::now(),
        );

        while !self.ins_state.state.exited {
            let step = self.ins_state.state.step;

            #[cfg(feature = "tracing")]
            if info_at(step) {
                let delta = start.elapsed();
                crate::traces::info!(
                    "[ELAPSED: {}.{:03}s] step: {}, pc: {}, instruction: {}, ips: {}, pages: {}, mem: {}",
                    delta.as_secs(),
                    delta.subsec_millis(),
                    step,
                    self.ins_state.state.pc,
                    self.ins_state.state.memory.get_memory(self.ins_state.state.pc)?,
                    (step - start_step) as f64 / delta.as_secs_f64(),
                    self.ins_state.state.memory.page_count(),
                    self.ins_state.state.memory.usage(),
                );
            }

            if stop_at(step) {
                break;
            }

            if snapshot_at(step) {
                let serialized_state = compress_bytes(&serde_json::to_vec(&self.ins_state.state)?)?;
                // TODO(clabby): Snapshot format option.
                fs::write(format!("{}", step), serialized_state)?;
            }

            if proof_at(step) {
                let prestate_hash = self.ins_state.state.encode_witness()?.state_hash();
                let step_witness = self
                    .ins_state
                    .step(true)?
                    .ok_or(anyhow!("No step witness"))?;
                let poststate_hash = self.ins_state.state.encode_witness()?.state_hash();

                // TODO: Reduce alloc.
                let mut proof = Proof {
                    step,
                    pre: prestate_hash,
                    post: poststate_hash,
                    state_data: step_witness.state.to_vec(),
                    proof_data: step_witness.mem_proof.clone(),
                    step_input: step_witness.encode_step_input().to_vec(),
                    ..Default::default()
                };

                if step_witness.has_preimage() {
                    let preimage_input = step_witness.encode_preimage_oracle_input();
                    proof.oracle_input = preimage_input.map(|k| k.to_vec());
                    proof.oracle_key = step_witness.preimage_key.map(|k| k.to_vec());
                    proof.oracle_value = step_witness.preimage_value;
                    proof.oracle_offset = step_witness.preimage_offset;
                }

                let serialized_proof = compress_bytes(&serde_json::to_vec(&proof)?)?;
                fs::write(format!("{}", step), serialized_proof)?;
            } else {
                self.ins_state.step(false)?;
            }
        }

        // Output the final state
        let serialized_state = serde_json::to_vec(&self.ins_state.state)?;
        if let Some(output) = &self.output {
            fs::write(output, compress_bytes(&serialized_state)?)?;
        } else {
            println!("{:?}", String::from_utf8(serialized_state));
        }

        // File descriptors are closed when the kernel struct is dropped, since it owns the oracle
        // server process and the preimage / hint writer clients.
        Ok(())
    }
}

/// Helper function to create a matcher function from a pattern.
///
/// - `never` will always return false
/// - `always` will always return true
/// - `=N` will return true when the step is equal to N
/// - `%N` will return true when the step is a multiple of N
fn create_matcher(pattern: Option<&String>) -> Result<Box<dyn Fn(u64) -> bool>> {
    match pattern {
        None => Ok(Box::new(|_| false)),
        Some(pattern) => match pattern.as_str() {
            "never" => Ok(Box::new(|_| false)),
            "always" => Ok(Box::new(|_| true)),
            _ if pattern.starts_with('=') => {
                // Extract the number from the pattern
                if let Ok(step) = pattern[1..].parse::<u64>() {
                    Ok(Box::new(move |s| s == step))
                } else {
                    anyhow::bail!("Invalid pattern: {}", pattern)
                }
            }
            _ if pattern.starts_with('%') => {
                // Extract the number from the pattern
                if let Ok(steps) = pattern[1..].parse::<u64>() {
                    Ok(Box::new(move |s| s % steps == 0))
                } else {
                    anyhow::bail!("Invalid pattern: {}", pattern)
                }
            }
            _ => {
                anyhow::bail!("Invalid pattern: {}", pattern)
            }
        },
    }
}
