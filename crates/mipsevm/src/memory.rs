//! The memory module contains the [Memory] data structure and its functionality for the emulator.

use crate::{
    page::{self, CachedPage},
    utils::concat_fixed,
};
use alloy_primitives::{keccak256, B256};
use anyhow::Result;
use fnv::FnvHashMap;
use std::{cell::RefCell, rc::Rc};

/// A [PageIndex] is
pub type PageIndex = u64;

/// A [Gindex] is a generalized index, defined as $2^{\text{depth}} + \text{index}$.
pub type Gindex = u64;

/// An [Address] is a 64 bit address in the MIPS emulator's memory.
pub type Address = u64;

/// The [Memory] struct represents the MIPS emulator's memory.
pub struct Memory {
    /// Map of generalized index -> the merkle root of each index. None if invalidated.
    nodes: FnvHashMap<Gindex, Option<B256>>,
    /// Map of page indices to [CachedPage]s.
    pages: FnvHashMap<PageIndex, Rc<RefCell<CachedPage>>>,
    /// We store two caches upfront; we often read instructions from one page and reserve another
    /// for scratch memory. This prevents map lookups for each instruction.
    last_page: [(PageIndex, Option<Rc<RefCell<CachedPage>>>); 2],
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            nodes: FnvHashMap::default(),
            pages: FnvHashMap::default(),
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
        for (key, page) in self.pages.iter() {
            f(*key, Rc::clone(page));
        }
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
        if let Some(page) = self.page_lookup(address >> page::PAGE_ADDRESS_SIZE) {
            let mut page = page.borrow_mut();
            page.invalidate(address & page::PAGE_ADDRESS_MASK as u64)?;
            if !page.is_valid(1) {
                return Ok(());
            }
        } else {
            // Nothing to invalidate
            return Ok(());
        }

        // Find the generalized index of the first page covering the address
        let mut g_index = ((1u64 << 32) | address) >> page::PAGE_ADDRESS_SIZE as u64;
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
    fn page_lookup(&mut self, page_index: PageIndex) -> Option<Rc<RefCell<CachedPage>>> {
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

    fn merklize_subtree(&mut self, g_index: Gindex) -> Result<B256> {
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
                    page.borrow_mut().merklize_subtree(page_g_index as usize)
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

        let left = self.merklize_subtree(g_index << 1)?;
        let right = self.merklize_subtree((g_index << 1) | 1)?;
        let result = keccak256(concat_fixed(left.into(), right.into()));

        self.nodes.insert(g_index, Some(result));

        Ok(result)
    }

    /// Compute the merkle root of the [Memory].
    ///
    /// ### Returns
    /// - The 32 byte merkle root hash of the [Memory].
    fn merkle_root(&mut self) -> Result<B256> {
        self.merklize_subtree(1)
    }

    /// Compute the merkle proof for the given address in the [Memory].
    ///
    /// ### Takes
    /// - `address`: The address to compute the merkle proof for.
    ///
    /// ### Returns
    /// - The 896 bit merkle proof for the given address.
    fn merkle_proof(&mut self, address: Address) -> Result<[u8; 28 << 5]> {
        let proof = self.traverse_branch(1, address, 0)?;
        let mut proof_out = [0u8; 28 << 5];

        // Encode the proof
        (0..28).for_each(|i| {
            let start = i << 5;
            proof_out[start..start + 32].copy_from_slice(proof[i].as_slice());
        });

        Ok(proof_out)
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
    fn traverse_branch(
        &mut self,
        parent: Gindex,
        address: Address,
        depth: u8,
    ) -> Result<Vec<B256>> {
        if depth == 32 - 5 {
            let mut proof = Vec::with_capacity(32 - 5 + 1);
            proof.push(self.merklize_subtree(parent)?);
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
        let sibling_node = self.merklize_subtree(sibling)?;
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
    fn set_memory(&mut self, address: Address, value: u32) -> Result<()> {
        // Address must be aligned to 4 bytes
        if address & 0x3 != 0 {
            anyhow::bail!("Unaligned memory access: {:x}", address);
        }

        let page_index = address >> page::PAGE_ADDRESS_SIZE as u64;
        let page_offset = address as usize & page::PAGE_ADDRESS_MASK;

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
        page.borrow_mut().data[page_offset..page_offset + 4].copy_from_slice(&value.to_be_bytes());

        Ok(())
    }

    /// Retrieve a 32 bit value from the [Memory] at a given address.
    ///
    /// ### Takes
    /// - `address`: The [Address] to retrieve the value from.
    ///
    /// ### Returns
    /// - The 32 bit value at the given address.
    fn get_memory(&mut self, address: Address) -> Result<u32> {
        // Address must be aligned to 4 bytes
        if address & 0x3 != 0 {
            anyhow::bail!("Unaligned memory access: {:x}", address);
        }

        if let Some(page) = self.page_lookup(address >> page::PAGE_ADDRESS_SIZE as u64) {
            let page_address = address as usize & page::PAGE_ADDRESS_MASK;
            Ok(u32::from_be_bytes(
                page.borrow().data[page_address..page_address + 4].try_into()?,
            ))
        } else {
            Ok(0)
        }
    }

    /// Allocate a new page in the [Memory] at a given page index.
    ///
    /// ### Takes
    /// - `page_index`: The page index to allocate the page at.
    ///
    /// ### Returns
    /// - A reference to the allocated [CachedPage].
    fn alloc_page(&mut self, page_index: PageIndex) -> Result<Rc<RefCell<CachedPage>>> {
        let page = Rc::new(RefCell::new(CachedPage::default()));
        self.pages.insert(page_index, Rc::clone(&page));

        let mut key = (1 << page::PAGE_KEY_SIZE) | page_index;
        while key > 0 {
            self.nodes.insert(key, None);
            key >>= 1;
        }
        Ok(page)
    }
}

#[cfg(test)]
mod test {
    use super::Memory;
    use crate::utils::concat_fixed;
    use alloy_primitives::{keccak256, B256};

    #[test]
    fn test_memory_merkle_proof() {
        let mut memory = Memory::default();
        memory.set_memory(0x10000, 0xaabbccdd).unwrap();
        let proof = memory.merkle_proof(0x10000).unwrap();
        assert_eq!([0xaa, 0xbb, 0xcc, 0xdd], proof[..4]);
        (0..32 - 5).for_each(|i| {
            let start = 32 + i * 32;
            assert_eq!(crate::page::ZERO_HASHES[i], proof[start..start + 32]);
        });

        let mut memory = Memory::default();
        memory.set_memory(0x10000, 0xaabbccdd).unwrap();
        memory.set_memory(0x80004, 42).unwrap();
        memory.set_memory(0x13370000, 123).unwrap();
        let root = memory.merkle_root().unwrap();
        let proof = memory.merkle_proof(0x80004).unwrap();
        assert_eq!([0x00, 0x00, 0x00, 0x2a], proof[4..8]);
        let mut node: B256 = proof[..32].try_into().unwrap();
        let mut path = 0x80004 >> 5;
        (32..proof.len()).step_by(32).for_each(|i| {
            let sib: B256 = proof[i..i + 32].try_into().unwrap();
            if path & 1 != 0 {
                node = keccak256(concat_fixed(sib.into(), node.into()));
            } else {
                node = keccak256(concat_fixed(node.into(), sib.into()));
            }
            path >>= 1;
        });
        assert_eq!(root, node, "proof must verify");
    }
}
