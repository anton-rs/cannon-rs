//! This module contains the [PreimageServer] struct and its associated methods.

use preimage_oracle::OracleClient;
use std::{io, os::fd::RawFd, path::PathBuf};

/// The [ProcessPreimageOracle] struct represents a preimage oracle process that communicates with
/// the mipsevm via a few special file descriptors. This process is responsible for preparing and
/// sending the results of preimage requests to the mipsevm process.
#[allow(dead_code)]
pub struct ProcessPreimageOracle {
    /// The preimage oracle client
    pub preimage_client: OracleClient,
    /// The hint writer client
    pub hint_writer_client: OracleClient,
}

impl ProcessPreimageOracle {
    /// Creates a new [PreimageServer] from the given [OracleClient]s.
    pub fn new(cmd: PathBuf, args: &[String]) -> Self {
        let fds = &[];

        let preimage_server = unsafe {
            tokio::process::Command::new(cmd)
                .args(args)
                .pre_exec(move || {
                    for (i, &fd) in fds.iter().enumerate() {
                        let new_fd = 3 + i as RawFd;
                        if libc::dup2(fd, new_fd) == -1 {
                            return Err(io::Error::last_os_error());
                        }
                    }
                    Ok(())
                })
        };

        todo!()
    }
}
