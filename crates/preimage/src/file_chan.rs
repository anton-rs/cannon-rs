//! This module contains the an implementation of the [FileChannel] trait in the form of
//! [ReadWritePair].

use crate::{traits::FileChannel, types::PreimageFds};
use anyhow::Result;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd};

/// A [ReadWritePair] represents a pair of file descriptors that can be used for reading and writing.
pub struct ReadWritePair {
    r: File,
    w: File,
}

impl ReadWritePair {
    pub fn new(r: File, w: File) -> Self {
        Self { r, w }
    }

    /// Helper to create a hinter channel.
    pub fn client_hinter_channel() -> ReadWritePair {
        let r = unsafe { File::from_raw_fd(PreimageFds::HintClientRead as i32) };
        let w = unsafe { File::from_raw_fd(PreimageFds::HintClientWrite as i32) };
        ReadWritePair::new(r, w)
    }

    /// Helper to create a preimage channel.
    pub fn client_preimage_channel() -> ReadWritePair {
        let r = unsafe { File::from_raw_fd(PreimageFds::PreimageClientRead as i32) };
        let w = unsafe { File::from_raw_fd(PreimageFds::PreimageClientWrite as i32) };
        ReadWritePair::new(r, w)
    }
}

impl Read for ReadWritePair {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.r.read(buf)
    }
}

impl Write for ReadWritePair {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.w.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

impl FileChannel for ReadWritePair {
    fn reader(&mut self) -> &mut File {
        &mut self.r
    }

    fn writer(&mut self) -> &mut File {
        &mut self.w
    }

    fn close(self) -> anyhow::Result<()> {
        // Self is dropped here, closing the file descriptors.
        Ok(())
    }
}

/// Helper to create a bidirectional channel through file descriptors opened by this process.
pub fn create_bidirectional_channel() -> Result<(ReadWritePair, ReadWritePair)> {
    let (ar, bw) = os_pipe::pipe()?;
    let (br, aw) = os_pipe::pipe()?;
    Ok((
        ReadWritePair::new(unsafe { File::from_raw_fd(ar.into_raw_fd()) }, unsafe {
            File::from_raw_fd(aw.into_raw_fd())
        }),
        ReadWritePair::new(unsafe { File::from_raw_fd(br.into_raw_fd()) }, unsafe {
            File::from_raw_fd(bw.into_raw_fd())
        }),
    ))
}
