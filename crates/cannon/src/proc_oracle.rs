//! This module contains the [PreimageServer] struct and its associated methods.

use anyhow::Result;
use cannon_mipsevm::PreimageOracle;
use preimage_oracle::{Hint, HintWriter, Hinter, Oracle, OracleClient};
use std::os::fd::AsRawFd;
use std::process::ExitStatus;
use std::{io, os::fd::RawFd, path::PathBuf};
use tokio::process::Child;

/// The [ProcessPreimageOracle] struct represents a preimage oracle process that communicates with
/// the mipsevm via a few special file descriptors. This process is responsible for preparing and
/// sending the results of preimage requests to the mipsevm process.
#[allow(dead_code)]
pub struct ProcessPreimageOracle {
    /// The preimage oracle client
    pub preimage_client: OracleClient,
    /// The hint writer client
    pub hint_writer_client: HintWriter,
    /// The preimage oracle server process
    pub server: Child,
}

impl ProcessPreimageOracle {
    /// Creates a new [PreimageServer] from the given [OracleClient] and [HintWriter] and starts
    /// the server process.
    pub fn start(cmd: PathBuf, args: &[String]) -> Result<Self> {
        let (hint_cl_rw, hint_oracle_rw) = preimage_oracle::create_bidirectional_channel()?;
        let (pre_cl_rw, pre_oracle_rw) = preimage_oracle::create_bidirectional_channel()?;

        unsafe {
            let mut cmd = tokio::process::Command::new(cmd);
            let cmd = cmd
                .args(args)
                .stdout(io::stdout())
                .stderr(io::stderr())
                .pre_exec(move || {
                    // Grab the file descriptors for the hint and preimage channels
                    // that the server will use to communicate with the mipsevm
                    let fds = &[
                        hint_oracle_rw.reader().as_raw_fd(),
                        hint_oracle_rw.writer().as_raw_fd(),
                        pre_oracle_rw.reader().as_raw_fd(),
                        pre_oracle_rw.writer().as_raw_fd(),
                    ];

                    // Pass along the file descriptors to the child process
                    for (i, &fd) in fds.iter().enumerate() {
                        let new_fd = 3 + i as RawFd;
                        if libc::dup2(fd, new_fd) == -1 {
                            return Err(io::Error::last_os_error());
                        }
                    }
                    Ok(())
                })
                .kill_on_drop(true);

            Ok(Self {
                preimage_client: OracleClient::new(pre_cl_rw),
                hint_writer_client: HintWriter::new(hint_cl_rw),
                server: cmd.spawn()?,
            })
        }
    }

    pub async fn wait(&mut self) -> Result<ExitStatus> {
        Ok(self.server.wait().await?)
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.server.kill().await?;
        Ok(())
    }
}

impl PreimageOracle for ProcessPreimageOracle {
    fn hint(&mut self, value: impl Hint) -> Result<()> {
        self.hint_writer_client.hint(value)
    }

    fn get(&mut self, key: [u8; 32]) -> anyhow::Result<Vec<u8>> {
        self.preimage_client.get(key)
    }
}
