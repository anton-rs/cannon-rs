//! This whole file needs rework.

use crate::FileChannel;
use anyhow::Result;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{task, time};

pub struct FilePoller {
    file: Arc<Mutex<dyn FileChannel + Send>>,
    poll_timeout: Duration,
    /// TODO(clabby): Baaaad way of doing this.
    cancellation_flag: Arc<AtomicBool>,
}

impl FilePoller {
    pub fn new(file: impl FileChannel + Send + 'static, poll_timeout: Duration) -> Self {
        Self {
            file: Arc::new(Mutex::new(file)),
            poll_timeout,
            cancellation_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn read(&self, buf: Arc<Mutex<Vec<u8>>>) -> Result<usize> {
        let mut read = 0;
        let buf_len = buf.lock().unwrap().len();

        loop {
            let read_future = task::spawn_blocking({
                let file = Arc::clone(&self.file);
                let buf_clone = Arc::clone(&buf);

                move || {
                    let mut file_lock = file
                        .lock()
                        .map_err(|e| anyhow::anyhow!("Failed to lock file: {}", e))?;
                    let mut buf_lock = buf_clone
                        .lock()
                        .map_err(|e| anyhow::anyhow!("Failed to lock buffer: {}", e))?;
                    file_lock
                        .read(&mut buf_lock[read..])
                        .map_err(|e| anyhow::anyhow!("Failed to read from file: {}", e))
                }
            });

            match time::timeout(self.poll_timeout, read_future).await {
                Ok(Ok(n)) => {
                    let n = n?;
                    read += n;
                    if read >= buf_len {
                        return Ok(read);
                    } else if n == 0 {
                        return Ok(buf_len);
                    }
                }
                Ok(Err(e)) => return Err(anyhow::anyhow!("{:?}", e)),
                Err(_) => {
                    if self
                        .cancellation_flag
                        .load(std::sync::atomic::Ordering::Relaxed)
                    {
                        return Err(anyhow::anyhow!("operation cancelled"));
                    }
                }
            }
        }
    }

    pub async fn write(&self, buf: Arc<Vec<u8>>) -> Result<usize> {
        let mut written = 0;
        let buf_len = buf.len();

        while written < buf_len {
            let write_future = task::spawn_blocking({
                let file = Arc::clone(&self.file);
                let buf_clone = Arc::clone(&buf);
                move || {
                    let mut file_lock = file
                        .lock()
                        .map_err(|e| anyhow::anyhow!("Failed to lock file: {e}"))?;
                    file_lock
                        .write(&buf_clone[written..])
                        .map_err(|e| anyhow::anyhow!("Failed to write to file: {e}"))
                }
            });

            match time::timeout(self.poll_timeout, write_future).await {
                Ok(Ok(n)) => {
                    let n = n?;
                    written += n;
                    if written >= buf_len {
                        return Ok(written);
                    } else if n == 0 {
                        return Ok(buf_len);
                    }
                }
                Ok(Err(e)) => return Err(anyhow::anyhow!("{:?}", e)),
                Err(_) => {
                    if self
                        .cancellation_flag
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        return Err(anyhow::anyhow!("operation cancelled"));
                    }
                }
            }
        }
        Ok(written)
    }

    pub async fn close(self) -> anyhow::Result<()> {
        // self will be dropped, dropping the `FileChannel` and closing the file descriptors
        // belonging to it.
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use tokio::try_join;

    use super::*;
    use std::io::{Read, Write};

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_read() {
        let (chan_a, mut chan_b) = crate::create_bidirectional_channel().unwrap();

        let poller = FilePoller::new(chan_a, Duration::from_millis(100));

        let r = tokio::task::spawn(async move {
            chan_b.write(b"hello").unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
            chan_b.write(b"world").unwrap();
        });

        let buf = Arc::new(Mutex::new(vec![0; 10]));
        let read = poller.read(buf).await.unwrap();
        assert_eq!(read, 10);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_write() {
        let (chan_a, mut chan_b) = crate::create_bidirectional_channel().unwrap();

        let poller = FilePoller::new(chan_a, Duration::from_millis(100));

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
        let written = poller.write(Arc::new(buf)).await.unwrap();
        assert_eq!(written, 10);

        try_join!(r).unwrap();
    }
}
