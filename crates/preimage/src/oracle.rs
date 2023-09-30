//! This module contains the [Client] struct and its implementation.

use crate::{Oracle, PreimageGetter, ReadWritePair};
use anyhow::Result;
use std::io::{Read, Write};

/// The [OracleClient] is a client that can make requests and write to the [OracleServer].
/// It contains a [ReadWritePair] that is one half of a bidirectional channel, with the other
/// half being owned by the [OracleServer].
pub struct OracleClient {
    io: ReadWritePair,
}

impl OracleClient {
    pub fn new(io: ReadWritePair) -> Self {
        Self { io }
    }
}

impl Oracle for OracleClient {
    fn get(&mut self, key: impl crate::Key) -> Result<Vec<u8>> {
        let hash = key.preimage_key();
        self.io.write_all(&hash)?;

        let mut length = [0u8; 8];
        self.io.read_exact(&mut length)?;
        let length = u64::from_be_bytes(length) as usize;

        let payload = if length == 0 {
            Vec::default()
        } else {
            let mut payload = vec![0u8; length];
            self.io.read_exact(&mut payload)?;
            payload
        };
        Ok(payload)
    }
}

/// The [OracleServer] is a server that can receive requests from the [OracleClient] and
/// respond to them. It contains a [ReadWritePair] that is one half of a bidirectional channel,
/// with the other half being owned by the [OracleClient].
pub struct OracleServer {
    io: ReadWritePair,
}

impl OracleServer {
    pub fn new(io: ReadWritePair) -> Self {
        Self { io }
    }
}

impl OracleServer {
    pub fn new_preimage_request(&mut self, getter: PreimageGetter) -> Result<()> {
        let mut key = [0u8; 32];
        self.io.read_exact(&mut key)?;

        let value = getter(key)?;

        self.io.write_all(&(value.len() as u64).to_be_bytes())?;
        if !value.is_empty() {
            self.io.write_all(&value)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{Oracle, OracleClient, OracleServer};
    use crate::{Keccak256Key, Key};
    use alloy_primitives::keccak256;
    use std::{collections::HashMap, sync::Arc};
    use tokio::sync::Mutex;

    async fn test_preimage(preimages: Vec<Vec<u8>>) {
        let (a, b) = crate::create_bidirectional_channel().unwrap();

        let client = Arc::new(Mutex::new(OracleClient::new(a)));
        let server = Arc::new(Mutex::new(OracleServer::new(b)));

        let mut preimage_by_hash: HashMap<[u8; 32], Vec<u8>> = Default::default();
        for preimage in preimages.iter() {
            let k = *keccak256(preimage) as Keccak256Key;
            preimage_by_hash.insert(k.preimage_key(), preimage.clone());
        }
        let preimage_by_hash = Arc::new(preimage_by_hash);

        for preimage in preimages.into_iter() {
            let k = *keccak256(preimage) as Keccak256Key;

            let client = Arc::clone(&client);
            let preimage_by_hash_a = Arc::clone(&preimage_by_hash);
            let join_a = tokio::task::spawn(async move {
                // Lock the client
                let mut cl = client.lock().await;
                let result = cl.get(k).unwrap();

                // Pull the expected value from the map
                let expected = preimage_by_hash_a.get(&k.preimage_key()).unwrap();
                assert_eq!(expected, &result);
            });

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            let server = Arc::clone(&server);
            let preimage_by_hash_b = Arc::clone(&preimage_by_hash);
            let join_b = tokio::task::spawn(async move {
                // Lock the server
                let mut server = server.lock().await;
                server
                    .new_preimage_request(Box::new(move |key: [u8; 32]| {
                        let dat = preimage_by_hash_b.get(&key).unwrap();
                        Ok(dat.clone())
                    }))
                    .unwrap();
            });

            tokio::try_join!(join_a, join_b).unwrap();
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn empty_preimage() {
        test_preimage(vec![vec![]]).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn zero() {
        test_preimage(vec![vec![0u8]]).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn multiple() {
        test_preimage(vec![
            b"tx from alice".to_vec(),
            vec![0x13, 0x37],
            b"tx from bob".to_vec(),
        ])
        .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn zeros() {
        test_preimage(vec![vec![0u8; 1000]]).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn random() {
        use rand::RngCore;

        let mut preimage = vec![0; 1000];
        rand::thread_rng().fill_bytes(&mut preimage[..]);

        test_preimage(vec![preimage]).await;
    }
}
