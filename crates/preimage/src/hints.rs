//! This module contains the [HintWriter] and [HintReader] structs and their implementations.

use crate::{types::HintHandler, Hint, Hinter};
use anyhow::Result;
use std::sync::mpsc::{Receiver, Sender};

/// The [HintWriter] sends hints to [HintReader] (e.g. a special file descriptor, or a debug log),
/// for a pre-image oracle service to prepare specific pre-images.
pub struct HintWriter {
    rx: Receiver<Vec<u8>>,
    tx: Sender<Vec<u8>>,
}

unsafe impl Send for HintWriter {}
unsafe impl Sync for HintWriter {}

impl HintWriter {
    fn new(rx: Receiver<Vec<u8>>, tx: Sender<Vec<u8>>) -> Self {
        Self { rx, tx }
    }
}

impl Hinter for HintWriter {
    fn hint<T: Hint>(&self, value: T) -> Result<()> {
        let hint = value.hint();
        let mut hint_bytes = vec![0u8; 4 + hint.len()];
        hint_bytes[0..4].copy_from_slice((hint.len() as u32).to_be_bytes().as_ref());
        hint_bytes[4..].copy_from_slice(hint);

        self.tx.send(hint_bytes)?;

        let n = self.rx.recv()?;
        if n.len() != 1 {
            anyhow::bail!(
                "Failed to read invalid pre-image hint ack, received response: {:?}",
                n
            );
        }
        Ok(())
    }
}

/// The [HintReader] reads hints from a [HintWriter] and prepares specific pre-images for
/// consumption by a pre-image oracle client.
pub struct HintReader {
    rx: Receiver<Vec<u8>>,
    tx: Sender<Vec<u8>>,
}

unsafe impl Send for HintReader {}
unsafe impl Sync for HintReader {}

impl HintReader {
    fn new(rx: Receiver<Vec<u8>>, tx: Sender<Vec<u8>>) -> Self {
        Self { rx, tx }
    }
}

impl HintReader {
    pub fn next_hint(&self, router: HintHandler) -> Result<bool> {
        let raw_payload = self.rx.recv()?;
        if raw_payload.len() < 4 {
            // Return EOF
            return Ok(true);
        }

        let length = u32::from_be_bytes(raw_payload.as_slice()[0..4].try_into()?) as usize;
        let payload = if length == 0 {
            Vec::default()
        } else {
            raw_payload[4..].try_into()?
        };

        if let Err(e) = router(&payload) {
            // Write back on error to unblock the hint writer.
            self.tx.send(vec![0])?;
            anyhow::bail!("Failed to handle hint: {:?}", e);
        }

        // write back to unblock the hint writer after routing the hint we received.
        self.tx.send(vec![0])?;
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        mpsc, Arc,
    };

    async fn test_hint(hints: Vec<Vec<u8>>) {
        let (bw, ar) = std::sync::mpsc::channel::<Vec<u8>>();
        let (aw, br) = std::sync::mpsc::channel::<Vec<u8>>();

        let hint_writer = Arc::new(HintWriter::new(ar, aw));
        let hint_reader = Arc::new(HintReader::new(br, bw));

        let counter_written = Arc::new(AtomicU32::new(0));
        let counter_received = Arc::new(AtomicU32::new(0));

        let (hints_a, counter_w) = (Arc::new(hints.clone()), Arc::clone(&counter_written));
        let a = tokio::spawn(async move {
            for hint in hints_a.iter() {
                counter_w.fetch_add(1, Ordering::SeqCst);
                hint_writer.hint(hint).unwrap();
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
                let Ok(eof) = reader.next_hint(Box::new(move |hint| {
                    // Increase the number of hint requests received.
                    counter_r.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })) else {
                    panic!("Failed to read hint {}", i);
                };

                if eof {
                    break;
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
        let (aw, br) = mpsc::channel();
        let (bw, ar) = mpsc::channel();

        let hint_writer = Arc::new(HintWriter::new(ar, aw));
        let hint_reader = Arc::new(HintReader::new(br, bw));

        let writer = Arc::clone(&hint_writer);
        let a = tokio::spawn(async move {
            writer.hint(b"one".to_vec().as_ref()).unwrap();
            writer.hint(b"two".to_vec().as_ref()).unwrap();
        });

        let reader = Arc::clone(&hint_reader);
        let b = tokio::spawn(async move {
            let Err(_) = reader.next_hint(Box::new(|hint| {
                anyhow::bail!("cb_error");
            })) else {
                panic!("Failed to read hint");
            };

            reader
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
