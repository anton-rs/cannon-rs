//! The memory module contains the [Memory] data structure and its functionality for the emulator.

use crate::{
    page::{self, CachedPage},
    utils::keccak_concat_fixed,
    Address, Gindex, PageIndex,
};
use anyhow::Result;
use rustc_hash::FxHashMap;
use std::{cell::RefCell, io::Read, rc::Rc};

/// The [Memory] struct represents the MIPS emulator's memory.
#[derive(Debug)]
pub struct Memory {
    /// Map of generalized index -> the merkle root of each index. None if invalidated.
    nodes: FxHashMap<Gindex, Option<[u8; 32]>>,
    /// Map of page indices to [CachedPage]s.
    pages: FxHashMap<PageIndex, Rc<RefCell<CachedPage>>>,
    /// We store two caches upfront; we often read instructions from one page and reserve another
    /// for scratch memory. This prevents map lookups for each instruction.
    last_page: [(PageIndex, Option<Rc<RefCell<CachedPage>>>); 2],
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            nodes: FxHashMap::default(),
            pages: FxHashMap::default(),
            last_page: [(!0u64, None), (!0u64, None)],
        }
    }
}

impl Memory {
    /// Returns the number of allocated pages in memory.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Performs an operation on all pages in the memory.
    ///
    /// ### Takes
    /// - `f`: A function that takes a [PageIndex] and a shared reference to a [CachedPage].
    pub fn for_each_page(&mut self, mut f: impl FnMut(PageIndex, Rc<RefCell<CachedPage>>)) {
        self.pages.iter().for_each(|(key, page)| {
            f(*key, Rc::clone(page));
        });
    }

    /// Invalidate a given memory address
    ///
    /// ### Takes
    /// - `address`: The address to invalidate.
    ///
    /// ### Returns
    /// - A [Result] indicating if the operation was successful.
    pub fn invalidate(&mut self, address: Address) -> Result<()> {
        if address & 0x3 != 0 {
            panic!("Unaligned memory access: {:x}", address);
        }

        // Find the page and invalidate the address within it.
        match self.page_lookup(address as u64 >> page::PAGE_ADDRESS_SIZE) {
            Some(page) => {
                let mut page = page.borrow_mut();
                let prev_valid = !page.valid[1];

                // Invalidate the address within the page.
                page.invalidate(address & page::PAGE_ADDRESS_MASK as u32)?;

                // If the page was already invalid before, then nodes to the memory
                // root will also still be invalid.
                if prev_valid {
                    return Ok(());
                }
            }
            None => {
                // Nothing to invalidate
                return Ok(());
            }
        }

        // Find the generalized index of the first page covering the address
        let mut g_index = ((1u64 << 32) | address as u64) >> page::PAGE_ADDRESS_SIZE;
        // Invalidate all nodes in the branch
        while g_index > 0 {
            self.nodes.insert(g_index, None);
            g_index >>= 1;
        }

        Ok(())
    }

    /// Lookup a page in the [Memory]. This function will consult the cache before checking the
    /// maps, and will cache the page if it is not already cached.
    ///
    /// ### Takes
    /// - `page_index`: The page index to look up.
    ///
    /// ### Returns
    /// - A reference to the [CachedPage] if it exists.
    pub fn page_lookup(&mut self, page_index: PageIndex) -> Option<Rc<RefCell<CachedPage>>> {
        // Check caches before maps
        if let Some((_, Some(page))) = self.last_page.iter().find(|(key, _)| *key == page_index) {
            Some(Rc::clone(page))
        } else if let Some(page) = self.pages.get(&page_index) {
            // Cache the page
            self.last_page[1] = self.last_page[0].clone();
            self.last_page[0] = (page_index, Some(Rc::clone(page)));

            Some(Rc::clone(page))
        } else {
            None
        }
    }

    pub fn merkleize_subtree(&mut self, g_index: Gindex) -> Result<[u8; 32]> {
        // Fetch the amount of bits required to represent the generalized index
        let bits = 64 - g_index.leading_zeros();
        if bits > 28 {
            anyhow::bail!("Gindex is too deep")
        }

        if bits > page::PAGE_KEY_SIZE as u32 {
            let depth_into_page = bits - 1 - page::PAGE_KEY_SIZE as u32;
            let page_index = (g_index >> depth_into_page) & page::PAGE_KEY_MASK as u64;
            return self.pages.get(&page_index).map_or(
                Ok(page::ZERO_HASHES[28 - bits as usize]),
                |page| {
                    let page_g_index =
                        (1 << depth_into_page) | (g_index & ((1 << depth_into_page) - 1));
                    page.borrow_mut().merkleize_subtree(page_g_index)
                },
            );
        }

        if bits > page::PAGE_KEY_SIZE as u32 + 1 {
            anyhow::bail!("Cannot jump into intermediate node of page")
        }

        if let Some(node) = self.nodes.get(&g_index) {
            if let Some(node) = node {
                return Ok(*node);
            }
        } else {
            return Ok(page::ZERO_HASHES[28 - bits as usize]);
        }

        let left = self.merkleize_subtree(g_index << 1)?;
        let right = self.merkleize_subtree((g_index << 1) | 1)?;
        let result = *keccak_concat_fixed(left, right);

        self.nodes.insert(g_index, Some(result));

        Ok(result)
    }

    /// Compute the merkle root of the [Memory].
    ///
    /// ### Returns
    /// - The 32 byte merkle root hash of the [Memory].
    pub fn merkle_root(&mut self) -> Result<[u8; 32]> {
        self.merkleize_subtree(1)
    }

    /// Compute the merkle proof for the given address in the [Memory].
    ///
    /// ### Takes
    /// - `address`: The address to compute the merkle proof for.
    ///
    /// ### Returns
    /// - The 896 bit merkle proof for the given address.
    pub fn merkle_proof(&mut self, address: Address) -> Result<[u8; 28 * 32]> {
        let proof = self.traverse_branch(1, address, 0)?;

        proof
            .into_iter()
            .flatten()
            .collect::<Vec<u8>>()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to convert proof to fixed array"))
    }

    /// Traverse a branch of the merkle tree, generating a proof for the given address.
    ///
    /// ### Takes
    /// - `parent`: The generalized index of the parent node.
    /// - `address`: The address to generate the proof for.
    /// - `depth`: The depth of the branch.
    ///
    /// ### Returns
    /// - The merkle proof for the given address.
    pub fn traverse_branch(
        &mut self,
        parent: Gindex,
        address: Address,
        depth: u8,
    ) -> Result<Vec<[u8; 32]>> {
        if depth == 32 - 5 {
            let mut proof = Vec::with_capacity(32 - 5 + 1);
            proof.push(self.merkleize_subtree(parent)?);
            return Ok(proof);
        }

        if depth > 32 - 5 {
            anyhow::bail!("Traversed too deep")
        }

        let mut local = parent << 1;
        let mut sibling = local | 1;
        if address & (1 << (31 - depth)) != 0 {
            (local, sibling) = (sibling, local);
        }

        let mut proof = self.traverse_branch(local, address, depth + 1)?;
        let sibling_node = self.merkleize_subtree(sibling)?;
        proof.push(sibling_node);

        Ok(proof)
    }

    /// Set a 32 bit value in the [Memory] at a given address.
    /// This will invalidate the page at the given address, or allocate a new page if it does not exist.
    ///
    /// ### Takes
    /// - `address`: The address to set the value at.
    /// - `value`: The 32 bit value to set.
    ///
    /// ### Returns
    /// - A [Result] indicating if the operation was successful.
    pub fn set_memory(&mut self, address: Address, value: u32) -> Result<()> {
        // Address must be aligned to 4 bytes
        if address & 0x3 != 0 {
            anyhow::bail!("Unaligned memory access: {:x}", address);
        }

        let page_index = address as PageIndex >> page::PAGE_ADDRESS_SIZE as u64;
        let page_address = address as usize & page::PAGE_ADDRESS_MASK;

        // Attempt to look up the page.
        // - If it does exist, invalidate it before changing it.
        // - If it does not exist, allocate it.
        let page = self
            .page_lookup(page_index)
            .map(|page| {
                // If the page exists, invalidate it - the value will change.
                self.invalidate(address)?;
                Ok::<_, anyhow::Error>(page)
            })
            .unwrap_or_else(|| self.alloc_page(page_index))?;

        // Copy the 32 bit value into the page
        page.borrow_mut().data[page_address..page_address + 4]
            .copy_from_slice(&value.to_be_bytes());

        Ok(())
    }

    /// Retrieve a 32 bit value from the [Memory] at a given address.
    ///
    /// ### Takes
    /// - `address`: The [Address] to retrieve the value from.
    ///
    /// ### Returns
    /// - The 32 bit value at the given address.
    pub fn get_memory(&mut self, address: Address) -> Result<u32> {
        // Address must be aligned to 4 bytes
        if address & 0x3 != 0 {
            anyhow::bail!("Unaligned memory access: {:x}", address);
        }

        match self.page_lookup(address as u64 >> page::PAGE_ADDRESS_SIZE as u64) {
            Some(page) => {
                let page_address = address as usize & page::PAGE_ADDRESS_MASK;
                Ok(u32::from_be_bytes(
                    page.borrow().data[page_address..page_address + 4].try_into()?,
                ))
            }
            None => Ok(0),
        }
    }

    /// Allocate a new page in the [Memory] at a given page index.
    ///
    /// ### Takes
    /// - `page_index`: The page index to allocate the page at.
    ///
    /// ### Returns
    /// - A reference to the allocated [CachedPage].
    pub fn alloc_page(&mut self, page_index: PageIndex) -> Result<Rc<RefCell<CachedPage>>> {
        let page = Rc::new(RefCell::new(CachedPage::default()));
        self.pages.insert(page_index, Rc::clone(&page));

        let mut key = (1 << page::PAGE_KEY_SIZE) | page_index;
        while key > 0 {
            self.nodes.insert(key, None);
            key >>= 1;
        }
        Ok(page)
    }

    /// Set a range of memory in the [Memory] at a given address.
    ///
    /// ### Takes
    /// - `address`: The address to set the memory at.
    /// - `data`: The data to set.
    ///
    /// ### Returns
    /// - A [Result] indicating if the operation was successful.
    pub fn set_memory_range<T: Read>(&mut self, address: Address, data: T) -> Result<()> {
        let mut address = address;
        let mut data = data;
        loop {
            let page_index = address as PageIndex >> page::PAGE_ADDRESS_SIZE as u64;
            let page_address = address as usize & page::PAGE_ADDRESS_MASK;

            let page = self
                .page_lookup(page_index)
                .map(Ok)
                .unwrap_or_else(|| self.alloc_page(page_index))?;
            page.borrow_mut().invalidate_full();

            match data.read(&mut page.borrow_mut().data[page_address..]) {
                Ok(n) => {
                    if n == 0 {
                        return Ok(());
                    }
                    address += n as u32;
                }
                Err(e) => return Err(e.into()),
            };
        }
    }
}

pub struct MemoryReader {
    memory: Rc<RefCell<Memory>>,
    address: Address,
    count: u32,
}

impl MemoryReader {
    pub fn new(memory: Rc<RefCell<Memory>>, address: Address, count: u32) -> Self {
        Self {
            memory,
            address,
            count,
        }
    }
}

impl Read for MemoryReader {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, std::io::Error> {
        if self.count == 0 {
            return Ok(0);
        }

        let end_address = self.address + self.count as Address;

        let page_index = self.address as PageIndex >> page::PAGE_ADDRESS_SIZE as u64;
        let start = self.address as usize & page::PAGE_ADDRESS_MASK;
        let mut end = page::PAGE_SIZE;

        if page_index == (end_address as u64 >> page::PAGE_ADDRESS_SIZE as u64) {
            end = end_address as usize & page::PAGE_ADDRESS_MASK;
        }
        let n = end - start;
        // TODO(clabby): To support reading the full stream, we should use something
        // less precise than `copy_from_slice`.
        match self.memory.borrow_mut().page_lookup(page_index) {
            Some(page) => {
                std::io::copy(&mut page.borrow().data[start..end].as_ref(), &mut buf)?;
            }
            None => {
                std::io::copy(&mut vec![0; n].as_slice(), &mut buf)?;
            }
        };
        self.address += n as u32;
        self.count -= n as u32;
        Ok(n)
    }
}

#[cfg(test)]
mod test {
    use super::Memory;
    use crate::{memory::Address, page, utils::keccak_concat_fixed};

    mod merkle_proof {
        use super::*;

        #[test]
        fn small_tree() {
            let mut memory = Memory::default();
            memory.set_memory(0x10000, 0xaabbccdd).unwrap();
            let proof = memory.merkle_proof(0x10000).unwrap();
            assert_eq!([0xaa, 0xbb, 0xcc, 0xdd], proof[..4]);
            (0..32 - 5).for_each(|i| {
                let start = 32 + i * 32;
                assert_eq!(crate::page::ZERO_HASHES[i], proof[start..start + 32]);
            });
        }

        #[test]
        fn larger_tree() {
            let mut memory = Memory::default();
            memory.set_memory(0x10000, 0xaabbccdd).unwrap();
            memory.set_memory(0x80004, 42).unwrap();
            memory.set_memory(0x13370000, 123).unwrap();
            let root = memory.merkle_root().unwrap();
            let proof = memory.merkle_proof(0x80004).unwrap();
            assert_eq!([0x00, 0x00, 0x00, 0x2a], proof[4..8]);
            let mut node = proof[..32].try_into().unwrap();
            let mut path = 0x80004 >> 5;
            (32..proof.len()).step_by(32).for_each(|i| {
                let sib: [u8; 32] = proof[i..i + 32].try_into().unwrap();
                if path & 1 != 0 {
                    node = *keccak_concat_fixed(sib, node);
                } else {
                    node = *keccak_concat_fixed(node, sib);
                }
                path >>= 1;
            });
            assert_eq!(root, node, "proof must verify");
        }
    }

    mod merkle_root {
        use super::*;

        #[test]
        fn empty() {
            let mut memory = Memory::default();
            let root = memory.merkle_root().unwrap();
            assert_eq!(
                crate::page::ZERO_HASHES[32 - 5],
                root,
                "Fully zeroed memory should have expected zero hash"
            );
        }

        #[test]
        fn empty_page() {
            let mut memory = Memory::default();
            memory.set_memory(0xF000, 0).unwrap();
            let root = memory.merkle_root().unwrap();
            assert_eq!(
                crate::page::ZERO_HASHES[32 - 5],
                root,
                "Fully zeroed memory should have expected zero hash"
            );
        }

        #[test]
        fn single_page() {
            let mut memory = Memory::default();
            memory.set_memory(0xF000, 1).unwrap();
            let root = memory.merkle_root().unwrap();
            assert_ne!(
                crate::page::ZERO_HASHES[32 - 5],
                root,
                "Non-zero memory should not have expected zero hash"
            );
        }

        #[test]
        fn repeat_zero() {
            let mut memory = Memory::default();
            memory.set_memory(0xF000, 0).unwrap();
            memory.set_memory(0xF004, 0).unwrap();
            let root = memory.merkle_root().unwrap();
            assert_eq!(
                crate::page::ZERO_HASHES[32 - 5],
                root,
                "Still should have expected zero hash"
            );
        }

        #[test]
        fn random_few_pages() {
            let mut memory = Memory::default();
            memory
                .set_memory(page::PAGE_SIZE as Address * 3, 1)
                .unwrap();
            memory
                .set_memory(page::PAGE_SIZE as Address * 5, 42)
                .unwrap();
            memory
                .set_memory(page::PAGE_SIZE as Address * 6, 123)
                .unwrap();
            let p3 = memory
                .merkleize_subtree((1 << page::PAGE_KEY_SIZE) | 3)
                .unwrap();
            let p5 = memory
                .merkleize_subtree((1 << page::PAGE_KEY_SIZE) | 5)
                .unwrap();
            let p6 = memory
                .merkleize_subtree((1 << page::PAGE_KEY_SIZE) | 6)
                .unwrap();
            let z = page::ZERO_HASHES[page::PAGE_ADDRESS_SIZE - 5];
            let r1 = keccak_concat_fixed(
                keccak_concat_fixed(
                    keccak_concat_fixed(z.into(), z.into()).into(),
                    keccak_concat_fixed(z.into(), p3.into()).into(),
                )
                .into(),
                keccak_concat_fixed(
                    keccak_concat_fixed(z.into(), p5.into()).into(),
                    keccak_concat_fixed(p6.into(), z.into()).into(),
                )
                .into(),
            );
            let r2 = memory
                .merkleize_subtree(1 << (page::PAGE_KEY_SIZE - 3))
                .unwrap();
            assert_eq!(
                r1, r2,
                "Expecting manual page combination to match subtree merkle func"
            );
        }

        #[test]
        fn invalidate_page() {
            let mut memory = Memory::default();
            memory.set_memory(0xF000, 0).unwrap();
            assert_eq!(
                crate::page::ZERO_HASHES[32 - 5],
                memory.merkle_root().unwrap(),
                "Zero at first"
            );
            memory.set_memory(0xF004, 1).unwrap();
            assert_ne!(
                crate::page::ZERO_HASHES[32 - 5],
                memory.merkle_root().unwrap(),
                "Non-zero"
            );
            memory.set_memory(0xF004, 0).unwrap();
            assert_eq!(
                crate::page::ZERO_HASHES[32 - 5],
                memory.merkle_root().unwrap(),
                "Zero again"
            );
        }
    }

    mod read_write {
        use super::*;
        use crate::memory::MemoryReader;
        use rand::RngCore;
        use std::{cell::RefCell, io::Read, rc::Rc};

        #[test]
        fn large_random() {
            let mut memory = Memory::default();
            let mut data = [0u8; 20_000];
            rand::thread_rng().fill_bytes(&mut data[..]);
            memory
                .set_memory_range(0, &data[..])
                .expect("Should not error");
            for i in [0, 4, 1000, 20_000 - 4] {
                let value = memory.get_memory(i).expect("Should not error");
                let expected =
                    u32::from_be_bytes(data[i as usize..i as usize + 4].try_into().unwrap());
                assert_eq!(expected, value, "read at {}", i);
            }
        }

        #[test]
        fn repeat_range() {
            let memory = Rc::new(RefCell::new(Memory::default()));
            let data = b"under the big bright yellow sun".repeat(40);
            memory
                .borrow_mut()
                .set_memory_range(0x1337, &data[..])
                .expect("Should not error");

            let mut reader =
                MemoryReader::new(Rc::clone(&memory), 0x1337 - 10, data.len() as u32 + 20);
            let mut buf = Vec::with_capacity(1260);
            reader.read_to_end(&mut buf).unwrap();

            assert_eq!([0u8; 10], buf[..10], "empty start");
            assert_eq!(data[..], buf[10..buf.len() - 10], "result");
            assert_eq!([0u8; 10], buf[buf.len() - 10..], "empty end");
        }

        #[test]
        fn read_write() {
            let mut memory = Memory::default();
            memory.set_memory(12, 0xaabbccdd).unwrap();
            assert_eq!(0xaabbccdd, memory.get_memory(12).unwrap());
            memory.set_memory(12, 0xaabbc1dd).unwrap();
            assert_eq!(0xaabbc1dd, memory.get_memory(12).unwrap());
        }

        #[test]
        fn unaligned_read() {
            let mut memory = Memory::default();
            memory.set_memory(12, 0xaabbccdd).unwrap();
            memory.set_memory(16, 0x11223344).unwrap();
            assert!(memory.get_memory(13).is_err());
            assert!(memory.get_memory(14).is_err());
            assert!(memory.get_memory(15).is_err());
            assert_eq!(0x11223344, memory.get_memory(16).unwrap());
            assert_eq!(0, memory.get_memory(20).unwrap());
            assert_eq!(0xaabbccdd, memory.get_memory(12).unwrap());
        }

        #[test]
        fn unaligned_write() {
            let mut memory = Memory::default();
            memory.set_memory(12, 0xaabbccdd).unwrap();
            assert!(memory.set_memory(13, 0x11223344).is_err());
            assert!(memory.set_memory(14, 0x11223344).is_err());
            assert!(memory.set_memory(15, 0x11223344).is_err());
            assert_eq!(0xaabbccdd, memory.get_memory(12).unwrap());
        }
    }
}
