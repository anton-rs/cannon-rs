//! This module contains the [Client] struct and its implementation.

use crate::{Oracle, PreimageGetter};
use alloy_primitives::B256;
use anyhow::Result;
use std::io::{Read, Write};

pub struct OracleClient<RW: Read + Write> {
    rw: RW,
}

impl<RW: Read + Write> OracleClient<RW> {
    fn new(rw: RW) -> Self {
        Self { rw }
    }
}

impl<RW: Read + Write> Oracle for OracleClient<RW> {
    fn get(&mut self, key: impl crate::Key) -> Result<Vec<u8>> {
        let hash = key.preimage_key();
        let _ = self.rw.write(hash.as_ref())?;

        let length = 0u64;
        let _ = self.rw.read(&mut length.to_be_bytes())?;

        let mut payload = vec![0u8; length as usize];
        let _ = self.rw.read_to_end(&mut payload)?;

        Ok(payload)
    }
}

pub struct OracleServer<RW: Read + Write> {
    rw: RW,
}

impl<RW: Read + Write> OracleServer<RW> {
    fn new(rw: RW) -> Self {
        Self { rw }
    }
}

impl<RW: Read + Write> OracleServer<RW> {
    pub fn new_preimage_request(&mut self, getter: PreimageGetter) -> Result<()> {
        let mut key = B256::ZERO;

        // TODO(clabby): Dunno if this is right.
        self.rw.read_exact(key.as_mut())?;

        let value = getter(key)?;

        let _ = self.rw.write(&(value.len() as u64).to_be_bytes())?;

        if value.is_empty() {
            return Ok(());
        }

        let _ = self.rw.write(value.as_ref())?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, io::Cursor, sync::Arc};

    use super::{Oracle, OracleClient, OracleServer};
    use crate::{Keccak256Key, Key, ReadWriterPair};
    use alloy_primitives::{keccak256, B256};
    use tokio::{join, sync::Mutex};

    async fn test_preimage(preimages: Vec<Vec<u8>>) {
        let (bw, ar) = (Cursor::new(Vec::default()), Cursor::new(Vec::default()));
        let (aw, br) = (Cursor::new(Vec::default()), Cursor::new(Vec::default()));

        let (a, b) = (ReadWriterPair::new(ar, aw), ReadWriterPair::new(br, bw));

        let client = Arc::new(Mutex::new(OracleClient::new(a)));
        let server = Arc::new(Mutex::new(OracleServer::new(b)));

        let mut preimage_by_hash: HashMap<B256, Vec<u8>> = Default::default();
        for preimage in preimages.iter() {
            let k = keccak256(preimage) as Keccak256Key;
            preimage_by_hash.insert(k.preimage_key(), preimage.clone());
        }
        let preimage_by_hash = Arc::new(preimage_by_hash);

        for preimage in preimages.into_iter() {
            let k = keccak256(preimage.as_slice()) as Keccak256Key;

            let client = Arc::clone(&client);
            let server = Arc::clone(&server);
            let preimage_by_hash_a = Arc::clone(&preimage_by_hash);
            let preimage_by_hash_b = Arc::clone(&preimage_by_hash);

            let join_a = tokio::task::spawn(async move {
                // Lock the client
                let mut cl = client.lock().await;
                let result = cl.get(k).unwrap();

                // Pull the expected value from the map
                let expected = preimage_by_hash_a.get(&k.preimage_key()).unwrap();
                assert_eq!(expected, &result);
            });

            let join_b = tokio::task::spawn(async move {
                // Lock the server
                let mut server = server.lock().await;
                server
                    .new_preimage_request(Box::new(move |key: B256| {
                        let dat = preimage_by_hash_b.get(&key).unwrap();
                        Ok(dat.clone())
                    }))
                    .unwrap();
            });

            let (ra, rb) = join!(join_a, join_b);
        }
    }

    #[tokio::test]
    async fn empty_preimage() {
        test_preimage(vec![vec![]]).await;
    }

    #[tokio::test]
    async fn zero() {
        test_preimage(vec![vec![0u8]]).await;
    }
}
