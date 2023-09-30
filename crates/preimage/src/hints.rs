//! This module contains the [HintWriter] and [HintReader] structs and their implementations.

use crate::{types::HintHandler, Hint, Hinter, ReadWritePair};
use anyhow::Result;
use std::io::{Read, Write};

/// The [HintWriter] sends hints to [HintReader] (e.g. a special file descriptor, or a debug log),
/// for a pre-image oracle service to prepare specific pre-images.
pub struct HintWriter {
    io: ReadWritePair,
}

unsafe impl Send for HintWriter {}
unsafe impl Sync for HintWriter {}

impl HintWriter {
    fn new(io: ReadWritePair) -> Self {
        Self { io }
    }
}

impl Hinter for HintWriter {
    fn hint<T: Hint>(&mut self, value: T) -> Result<()> {
        let hint = value.hint();
        let mut hint_bytes = vec![0u8; 4 + hint.len()];
        hint_bytes[0..4].copy_from_slice((hint.len() as u32).to_be_bytes().as_ref());
        hint_bytes[4..].copy_from_slice(hint);

        crate::debug!("Sending hint: {:?}", hint_bytes);
        self.io.write(&hint_bytes)?;

        self.io.read_exact(&mut [0])?;
        Ok(())
    }
}

/// The [HintReader] reads hints from a [HintWriter] and prepares specific pre-images for
/// consumption by a pre-image oracle client.
pub struct HintReader {
    io: ReadWritePair,
}

unsafe impl Send for HintReader {}
unsafe impl Sync for HintReader {}

impl HintReader {
    fn new(io: ReadWritePair) -> Self {
        Self { io }
    }
}

impl HintReader {
    pub fn next_hint(&mut self, router: HintHandler) -> Result<bool> {
        let mut length = [0u8; 4];
        let n = self.io.read(&mut length)?;
        if n < 4 {
            // Return EOF
            return Ok(true);
        }

        let length = u32::from_be_bytes(length) as usize;
        let payload = if length == 0 {
            Vec::default()
        } else {
            let mut raw_payload = vec![0u8; length];
            self.io.read_exact(&mut raw_payload)?;
            raw_payload
        };

        if let Err(e) = router(&payload) {
            // Write back on error to unblock the hint writer.
            self.io.write(&[0])?;
            crate::error!("Failed to handle hint: {:?}", e);
            anyhow::bail!("Failed to handle hint: {:?}", e);
        }

        // write back to unblock the hint writer after routing the hint we received.
        self.io.write(&[0])?;
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };
    use tokio::sync::Mutex;

    async fn test_hint(hints: Vec<Vec<u8>>) {
        let (a, b) = crate::create_bidirectional_channel().unwrap();

        let hint_writer = Arc::new(Mutex::new(HintWriter::new(a)));
        let hint_reader = Arc::new(Mutex::new(HintReader::new(b)));

        let counter_written = Arc::new(AtomicU32::new(0));
        let counter_received = Arc::new(AtomicU32::new(0));

        let (hints_a, counter_w) = (Arc::new(hints.clone()), Arc::clone(&counter_written));
        let a = tokio::spawn(async move {
            for hint in hints_a.iter() {
                counter_w.fetch_add(1, Ordering::SeqCst);
                hint_writer.lock().await.hint(hint).unwrap();
            }
        });

        let (reader, hints_b, counter_r) = (
            Arc::clone(&hint_reader),
            Arc::new(hints.clone()),
            Arc::clone(&counter_received),
        );
        let b = tokio::spawn(async move {
            for i in 0..hints_b.len() {
                let counter_r = Arc::clone(&counter_r);
                match reader.lock().await.next_hint(Box::new(move |hint| {
                    // Increase the number of hint requests received.
                    counter_r.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })) {
                    Ok(eof) => {
                        if eof {
                            break;
                        }
                    }
                    Err(e) => panic!("Failed to read hint {}", e),
                }
            }
        });

        tokio::try_join!(a, b).unwrap();

        assert_eq!(
            hints.len(),
            counter_received.load(Ordering::SeqCst) as usize
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn empty_hint() {
        test_hint(vec![vec![]]).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hello_world_hint() {
        test_hint(vec![b"hello world".to_vec()]).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn zero_byte() {
        test_hint(vec![vec![0]]).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn many_zeros() {
        test_hint(vec![vec![0; 1000]]).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn rand_bytes() {
        use rand::RngCore;

        let mut rand = [0u8; 2048];
        rand::thread_rng().fill_bytes(&mut rand);
        test_hint(vec![rand.to_vec()]).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn multiple_hints() {
        test_hint(vec![
            b"hello world".to_vec(),
            b"cannon cannon cannon".to_vec(),
            b"".to_vec(),
            b"milady".to_vec(),
        ])
        .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cb_error() {
        let (a, b) = crate::create_bidirectional_channel().unwrap();

        let hint_writer = Arc::new(Mutex::new(HintWriter::new(a)));
        let hint_reader = Arc::new(Mutex::new(HintReader::new(b)));

        let writer = Arc::clone(&hint_writer);
        let a = tokio::spawn(async move {
            let mut writer_lock = writer.lock().await;
            writer_lock.hint(b"one".to_vec().as_ref()).unwrap();
            writer_lock.hint(b"two".to_vec().as_ref()).unwrap();
        });

        let reader = Arc::clone(&hint_reader);
        let b = tokio::spawn(async move {
            let mut reader_lock = reader.lock().await;

            let Err(_) = reader_lock.next_hint(Box::new(|hint| {
                anyhow::bail!("cb_error");
            })) else {
                panic!("Failed to read hint");
            };

            reader_lock
                .next_hint(Box::new(|hint| {
                    assert_eq!(hint, b"two");
                    Ok(())
                }))
                .unwrap();
        });
    }

    impl Hint for String {
        fn hint(&self) -> &[u8] {
            self.as_bytes()
        }
    }
    impl Hint for &Vec<u8> {
        fn hint(&self) -> &[u8] {
            self.as_slice()
        }
    }
}
