// //! This module contains the [Client] struct and its implementation.
//
// use crate::{Oracle, PreimageGetter};
// use alloy_primitives::B256;
// use anyhow::Result;
// use std::io::{Read, Write};
//
// pub struct OracleClient<RW: Read + Write> {
//     rw: RW,
// }
//
// impl<RW: Read + Write> OracleClient<RW> {
//     fn new(rw: RW) -> Self {
//         Self { rw }
//     }
// }
//
// impl<RW: Read + Write> Oracle for OracleClient<RW> {
//     fn get(&mut self, key: impl crate::Key) -> Result<Vec<u8>> {
//         let hash = key.preimage_key();
//         self.rw.write(hash.as_ref())?;
//
//         let length = 0u64;
//         self.rw.read(&mut length.to_be_bytes())?;
//
//         let mut payload = vec![0u8; length as usize];
//         self.rw.read_to_end(&mut payload)?;
//
//         Ok(payload)
//     }
// }
//
// pub struct OracleServer<RW: Read + Write> {
//     rw: RW,
// }
//
// impl<RW: Read + Write> OracleServer<RW> {
//     fn new(rw: RW) -> Self {
//         Self { rw }
//     }
// }
//
// impl<RW: Read + Write> OracleServer<RW> {
//     pub fn new_preimage_request(&mut self, getter: PreimageGetter) -> Result<()> {
//         let mut key = B256::ZERO;
//
//         // TODO(clabby): Dunno if this is right.
//         self.rw.read_exact(key.as_mut())?;
//
//         let value = getter(key)?;
//
//         self.rw.write(&(value.len() as u64).to_be_bytes())?;
//
//         if value.is_empty() {
//             return Ok(());
//         }
//
//         self.rw.write(value.as_ref())?;
//
//         Ok(())
//     }
// }
//
// #[cfg(test)]
// mod test {}
