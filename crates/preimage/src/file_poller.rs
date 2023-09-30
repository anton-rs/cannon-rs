//! This module contains the [FilePoller] and its implementation.

use crate::FileChannel;
use anyhow::Result;

/// A [FilePoller] represents a pair of file descriptors that can be used for reading and writing.
pub struct FilePoller {
    file: Box<dyn FileChannel + Send>,
}

// TODO(clabby): Temp; Bring to spec of the `Read` + `Write` traits.
impl FilePoller {
    pub fn new(file: impl FileChannel + Send + 'static) -> Self {
        Self {
            file: Box::new(file),
        }
    }

    /// Read from the file into `buf` until `buf` is full or EOF is reached.
    ///
    /// ### Takes
    /// - `buf`: The buffer to read into.
    ///
    /// ### Returns
    /// - `Ok(usize)`: The number of bytes read.
    /// - `Err(anyhow::Error)`: An error occurred while reading.
    pub fn read(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        let mut read = 0;
        loop {
            match self.file.read(&mut buf[read..]) {
                Ok(n) => {
                    read += n;
                    if n == 0 || read >= buf.len() {
                        // 0 bytes read may indicate EOF or close signal
                        return Ok(read);
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("Failed to read from file: {}", e)),
            }
        }
    }

    /// Write the entire contents of `buf` to the file.
    ///
    /// ### Takes
    /// - `buf`: The buffer to write.
    ///
    /// ### Returns
    /// - `Ok(usize)`: The number of bytes written.
    /// - `Err(anyhow::Error)`: An error occurred while writing.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut written = 0;
        loop {
            match self.file.write(&buf[written..]) {
                Ok(n) => {
                    written += n;
                    if n == 0 || written >= buf.len() {
                        // 0 bytes written may indicate EOF or close signal
                        return Ok(written);
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("Failed to write to file: {}", e)),
            }
        }
    }

    pub fn close(self) -> anyhow::Result<()> {
        // self will be dropped, dropping the `FileChannel` and closing the file descriptors
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{
        io::{Read, Write},
        sync::Arc,
        time::Duration,
    };
    use tokio::{sync::Mutex, try_join};

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn read() {
        let (chan_a, mut chan_b) = crate::create_bidirectional_channel().unwrap();

        let mut poller = FilePoller::new(chan_a);

        let r = tokio::task::spawn(async move {
            chan_b.write(b"hello").unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
            chan_b.write(b"world").unwrap();
        });

        let mut buf = vec![0; 10];
        let read = poller.read(&mut buf).unwrap();
        assert_eq!(read, 10);

        try_join!(r).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn write() {
        let (chan_a, mut chan_b) = crate::create_bidirectional_channel().unwrap();

        let mut poller = FilePoller::new(chan_a);

        let r = tokio::task::spawn(async move {
            let mut buf = vec![0; 10];
            chan_b.read(&mut buf[..5]).unwrap();
            assert_eq!(&buf[..5], b"hello");
            tokio::time::sleep(Duration::from_secs(1)).await;
            chan_b.read(&mut buf[5..]).unwrap();
            assert_eq!(&buf[5..], b"world");

            assert_eq!("helloworld", String::from_utf8(buf).unwrap());
        });

        let buf = b"helloworld".to_vec();
        let written = poller.write(&buf).unwrap();
        assert_eq!(written, 10);

        try_join!(r).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn read_cancelled() {
        let (chan_a, mut chan_b) = crate::create_bidirectional_channel().unwrap();
        let mut poller = FilePoller::new(chan_a);

        let r = tokio::task::spawn(async move {
            chan_b.write(b"hello").unwrap();
        });

        let mut buf = vec![0; 10];
        let read = poller.read(&mut buf).unwrap();
        assert_eq!(read, 5);
        assert_eq!(buf[..5], b"hello"[..]);
        assert_eq!(buf[5..], [0; 5]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn write_cancelled() {
        let (chan_a, mut chan_b) = crate::create_bidirectional_channel().unwrap();
        let mut poller = FilePoller::new(chan_a);

        let read_buf = Arc::new(Mutex::new(vec![0; 5]));
        let read_buf_shared = Arc::clone(&read_buf);
        let r = tokio::task::spawn(async move {
            let mut buf = read_buf_shared.lock().await;
            chan_b.read(&mut buf).unwrap();
        });

        tokio::time::sleep(Duration::from_secs(1)).await;
        let buf = b"helloworld".to_vec();
        let written = poller.write(&buf).unwrap();
        assert_eq!(written, 10);
        assert_eq!(buf[..5], read_buf.lock().await[..5]);

        try_join!(r).unwrap();
    }
}
