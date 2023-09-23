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

impl HintWriter {
    fn new(rx: Receiver<Vec<u8>>, tx: Sender<Vec<u8>>) -> Self {
        Self { rx, tx }
    }
}

impl Hinter for HintWriter {
    fn hint<T: Hint>(&self, value: T) -> Result<bool> {
        let hint = value.hint();
        let mut hint_bytes = vec![0u8; 4 + hint.len()];
        hint_bytes[0..4].copy_from_slice((hint.len() as u32).to_be_bytes().as_ref());
        hint_bytes[4..].copy_from_slice(hint);

        self.tx.send(hint_bytes)?;

        match self.rx.recv() {
            Ok(n) => {
                if n.len() != 1 {
                    anyhow::bail!(
                        "Failed to read invalid pre-image hint ack, received response: {:?}",
                        n
                    );
                }
                Ok(true)
            }
            Err(e) => Ok(false),
        }
    }
}

/// The [HintReader] reads hints from a [HintWriter] and prepares specific pre-images for
/// consumption by a pre-image oracle client.
pub struct HintReader {
    rx: Receiver<Vec<u8>>,
    tx: Sender<Vec<u8>>,
}

impl HintReader {
    fn new(rx: Receiver<Vec<u8>>, tx: Sender<Vec<u8>>) -> Self {
        Self { rx, tx }
    }
}

impl HintReader {
    pub fn next_hint(&self, router: HintHandler) -> Result<bool> {
        let raw_len = self.rx.recv()?;
        if raw_len.len() != 4 {
            return Ok(true);
        }
        let length = u32::from_be_bytes(raw_len.as_slice().try_into()?) as usize;
        let payload = if length == 0 {
            Vec::default()
        } else {
            self.rx.recv()?
        };

        if let Err(e) = router(&payload) {
            // Write back on error to unblock the hint writer.
            self.tx.send(vec![0])?;
            anyhow::bail!("Failed to handle hint: {:?}", e);
        }

        // write back to unblock the hint writer after routing the hint we received.
        self.tx.send(vec![0])?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    async fn test_hint(hints: Vec<Vec<u8>>) {
        let (bw, ar) = std::sync::mpsc::channel::<Vec<u8>>();
        let (aw, br) = std::sync::mpsc::channel::<Vec<u8>>();

        let counter_written = Arc::new(AtomicU32::new(0));
        let counter_received = Arc::new(AtomicU32::new(0));

        let hints_a = Arc::new(hints.clone());
        let counter_w = Arc::clone(&counter_written);
        let a = tokio::spawn(async move {
            let hint_writer = HintWriter::new(ar, aw);
            let cw = Arc::clone(&counter_w);
            for hint in hints_a.iter() {
                cw.fetch_add(1, Ordering::SeqCst);
                hint_writer.hint(hint).unwrap();
            }
        });

        let hints_b = Arc::new(hints.clone());
        let counter_r = Arc::clone(&counter_received);
        let b = tokio::spawn(async move {
            let hint_reader = HintReader::new(br, bw);
            for i in 0..hints_b.len() {
                let counter = Arc::clone(&counter_r);
                let Ok(eof) = hint_reader.next_hint(Box::new(move |hint| {
                    // Increase the number of hint requests received.
                    counter.fetch_add(1, Ordering::SeqCst);
                    dbg!("yo");
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
        test_hint(vec![b"asd".to_vec()]).await;
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
