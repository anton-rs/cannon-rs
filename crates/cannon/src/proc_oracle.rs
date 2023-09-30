//! This module contains the [PreimageServer] struct and its associated methods.

use cannon_mipsevm::PreimageOracle;
use preimage_oracle::{FilePoller, OracleClient};
use std::os::fd::AsRawFd;
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
        let (hint_cl_rw, hint_oracle_rw) =
            preimage_oracle::create_bidirectional_channel().expect("Failed to create hint channel");
        let (pre_cl_rw, pre_oracle_rw) = preimage_oracle::create_bidirectional_channel()
            .expect("Failed to create preimage channel");

        let preimage_server = unsafe {
            tokio::process::Command::new(cmd)
                .args(args)
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
        };

        // let preimage_client_io = preimage_oracle::FilePoller::new(pre_cl_rw);
        // let host_client_io = preimage_oracle::FilePoller::new(hint_cl_rw);
        Self {
            preimage_client: OracleClient::new(pre_cl_rw),
            hint_writer_client: OracleClient::new(hint_cl_rw),
        }
    }
}

impl PreimageOracle for ProcessPreimageOracle {
    fn hint(&mut self, value: &[u8]) {
        todo!()
    }

    fn get(&self, key: [u8; 32]) -> anyhow::Result<&[u8]> {
        todo!()
    }
}
