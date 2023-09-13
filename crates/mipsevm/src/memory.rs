//! The memory module contains the memory data structures and functionality for the emulator.

use crate::page::{self, CachedPage};
use alloy_primitives::B256;
use anyhow::Result;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

type PageIndex = u64;

/// The [Memory] struct represents the MIPS emulator's memory.
struct Memory {
    /// Map of generalized index -> the merkle root of each index. None if invalidated.
    nodes: BTreeMap<u64, Option<B256>>,
    /// Map of page indices to [CachedPage]s.
    pages: BTreeMap<u64, Rc<RefCell<CachedPage>>>,
    /// We store two caches upfront; we often read instructions from one page and reserve another
    /// for scratch memory. This prevents map lookups for each instruction.
    last_page: [(PageIndex, Option<Rc<RefCell<CachedPage>>>); 2],
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::default(),
            pages: BTreeMap::default(),
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
}

#[cfg(test)]
mod test {}
