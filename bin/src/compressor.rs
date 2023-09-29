//! This module contains utilities for compressing and decompressing serialized bytes.

use anyhow::Result;
use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use std::io::{Read, Write};

/// Compresses a byte slice using gzip.
pub(crate) fn compress_bytes(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}

/// Decompresses a byte slice using gzip.
pub(crate) fn decompress_bytes(compressed_bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(compressed_bytes);

    // Give our decompressed buffer the same capacity as the compressed buffer to reduce
    // reallocations up to the compressed buffer's size.
    let mut decompressed_bytes = Vec::with_capacity(compressed_bytes.len());
    decoder.read_to_end(&mut decompressed_bytes)?;

    Ok(decompressed_bytes)
}

#[cfg(test)]
mod test {
    use proptest::proptest;

    proptest! {
        #[test]
        fn test_compress_decompress(bytes: Vec<u8>) {
            let compressed = super::compress_bytes(&bytes).unwrap();
            let decompressed = super::decompress_bytes(&compressed).unwrap();
            assert_eq!(bytes, decompressed);
        }
    }
}
