use crate::FileChannel;
use anyhow::Result;
use bytes::{Bytes, BytesMut};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{task, time};

pub struct FilePoller {
    file: Arc<Mutex<dyn FileChannel + Send>>,
    poll_timeout: Duration,
}

impl FilePoller {
    pub fn new(file: impl FileChannel + Send + 'static, poll_timeout: Duration) -> Self {
        Self {
            file: Arc::new(Mutex::new(file)),
            poll_timeout,
        }
    }

    pub async fn read(&self, mut buf: BytesMut) -> Result<usize> {
        let file = Arc::clone(&self.file);
        let read_future = task::spawn_blocking(move || {
            let mut file_lock = file
                .lock()
                .map_err(|e| anyhow::anyhow!("Error locking file: {}", e))?;
            file_lock
                .read(&mut buf)
                .map_err(|e| anyhow::anyhow!("Error reading file: {}", e))
        });
        match time::timeout(self.poll_timeout, read_future).await {
            Ok(result) => result?,
            Err(_) => anyhow::bail!("operation timed out"),
        }
    }

    pub async fn write(&self, buf: Bytes) -> Result<usize> {
        let file = Arc::clone(&self.file);
        let write_future = task::spawn_blocking(move || {
            let mut file_lock = file
                .lock()
                .map_err(|e| anyhow::anyhow!("Error locking file: {}", e))?;
            file_lock
                .write(&buf)
                .map_err(|e| anyhow::anyhow!("Error writing file: {}", e))
        });
        match time::timeout(self.poll_timeout, write_future).await {
            Ok(result) => result.unwrap(),
            Err(_) => anyhow::bail!("operation timed out"),
        }
    }

    pub async fn close(self) -> anyhow::Result<()> {
        // self will be dropped, dropping the `FileChannel` and closing the file descriptors
        // belonging to it.
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Write;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore]
    async fn test_read() {
        let (chan_a, mut chan_b) = crate::create_bidirectional_channel().unwrap();

        let poller = FilePoller::new(chan_a, Duration::from_millis(100));

        tokio::task::spawn(async move {
            chan_b.write(b"hello").unwrap();
        });

        let buf = BytesMut::new();
        let read = poller.read(buf).await.unwrap();
        assert_eq!(read, 10);
    }
}
