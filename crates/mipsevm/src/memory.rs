//! The memory module contains the memory data structures and functionality for the emulator.

use crate::{
    page::{self, CachedPage},
    utils::concat_fixed,
};
use alloy_primitives::{keccak256, B256};
use anyhow::Result;
use fnv::FnvHashMap;
use std::{cell::RefCell, rc::Rc};

type PageIndex = u64;

/// The [Memory] struct represents the MIPS emulator's memory.
struct Memory {
    /// Map of generalized index -> the merkle root of each index. None if invalidated.
    nodes: FnvHashMap<u64, Option<B256>>,
    /// Map of page indices to [CachedPage]s.
    pages: FnvHashMap<u64, Rc<RefCell<CachedPage>>>,
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
    /// - `f`: A function that takes a page index and a mutable reference to a [CachedPage].
    pub fn for_each_page(&mut self, mut f: impl FnMut(u64, Rc<RefCell<CachedPage>>)) {
        for (key, page) in self.pages.iter() {
            f(*key, Rc::clone(page));
        }
    }

    /// Invalidate a given memory address
    ///
    /// ### Takes
    /// - `address`: The address to invalidate.
    pub fn invalidate(&mut self, address: u64) -> Result<()> {
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

    fn merklize_subtree(&mut self, g_index: u64) -> Result<B256> {
        // Fetch the amount of bits required to represent the generalized index
        let bits = 128 - g_index.leading_zeros();
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
    fn merkle_proof(&mut self, address: u32) -> Result<[u8; 28 << 5]> {
        let proof = self.traverse_branch(1, address, 0)?;
        let mut proof_out = [0u8; 28 << 5];

        // Encode the proof
        (0..28).for_each(|i| {
            let start = i << 5;
            proof_out[start..start + 32].copy_from_slice(proof[i].as_slice());
        });

        Ok(proof_out)
    }

    fn traverse_branch(&mut self, parent: u64, address: u32, depth: u8) -> Result<Vec<B256>> {
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

    fn set_memory(&mut self, address: u64, value: u32) -> Result<()> {
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
    /// - `address`: The address to retrieve the value from.
    ///
    /// ### Returns
    /// - The 32 bit value at the given address.
    fn get_memory(&mut self, address: u64) -> Result<u32> {
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
    fn alloc_page(&mut self, page_index: u64) -> Result<Rc<RefCell<CachedPage>>> {
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
mod test {}
