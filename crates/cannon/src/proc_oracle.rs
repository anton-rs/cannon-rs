//! This module contains the [PreimageServer] struct and its associated methods.

use anyhow::Result;
use cannon_mipsevm::PreimageOracle;
use command_fds::{CommandFdExt, FdMapping};
use preimage_oracle::{Hint, HintWriter, Hinter, Oracle, OracleClient, ReadWritePair};
use std::{
    io,
    os::fd::AsRawFd,
    path::PathBuf,
    process::{Child, Command},
};

/// The [ProcessPreimageOracle] struct represents a preimage oracle process that communicates with
/// the mipsevm via a few special file descriptors. This process is responsible for preparing and
/// sending the results of preimage requests to the mipsevm process.
pub struct ProcessPreimageOracle {
    /// The preimage oracle client
    pub preimage_client: OracleClient,
    /// The hint writer client
    pub hint_writer_client: HintWriter,
}

impl ProcessPreimageOracle {
    /// Creates a new [PreimageServer] from the given [OracleClient] and [HintWriter] and starts
    /// the server process.
    pub fn start(
        cmd: PathBuf,
        args: &[String],
        client_io: (ReadWritePair, ReadWritePair),
        server_io: &[ReadWritePair; 2],
    ) -> Result<(Self, Option<Child>)> {
        let cmd_str = cmd.display().to_string();
        let child = (!cmd_str.is_empty()).then(|| {
            crate::info!(
                "Starting preimage server process: {} {:?}",
                cmd.display(),
                args
            );

            let mut command = Command::new(cmd);
            let command = {
                // Grab the file descriptors for the hint and preimage channels
                // that the server will use to communicate with the mipsevm
                let fds = [
                    server_io[0].reader().as_raw_fd(),
                    server_io[0].writer().as_raw_fd(),
                    server_io[1].reader().as_raw_fd(),
                    server_io[1].writer().as_raw_fd(),
                ];

                crate::traces::info!(target: "cannon::preimage::server", "Starting preimage server process: {:?} with fds {:?}", args, fds);

                command
                    .args(args)
                    .stdout(io::stdout())
                    .stderr(io::stderr())
                    .fd_mappings(
                        fds.iter().enumerate()
                            .map(|(i, fd)| FdMapping {
                                parent_fd: *fd,
                                child_fd: 3 + i as i32,
                            })
                            .collect(),
                    )?
            };

            command.spawn().map_err(|e| anyhow::anyhow!("Failed to start preimage server process: {}", e))
        });

        Ok((
            Self {
                hint_writer_client: HintWriter::new(client_io.0),
                preimage_client: OracleClient::new(client_io.1),
            },
            child.transpose()?,
        ))
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
