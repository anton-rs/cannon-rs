//! This module contains the data structure for a [Page] within the
//! MIPS emulator's memory.

use crate::utils::concat_fixed;
use alloy_primitives::{keccak256, B256};
use once_cell::sync::Lazy;

pub(crate) const PAGE_ADDRESS_SIZE: usize = 12;
pub(crate) const PAGE_KEY_SIZE: usize = 32 - PAGE_ADDRESS_SIZE;
pub(crate) const PAGE_SIZE: usize = 1 << PAGE_ADDRESS_SIZE;
pub(crate) const PAGE_ADDRESS_MASK: usize = PAGE_SIZE - 1;
pub(crate) const MAX_PAGE_COUNT: usize = 1 << PAGE_KEY_SIZE;
pub(crate) const PAGE_KEY_MASK: usize = MAX_PAGE_COUNT - 1;

/// Precomputed hashes of each full-zero range sub-tree level.
pub(crate) static ZERO_HASHES: Lazy<[B256; 256]> = Lazy::new(|| {
    let mut out = [B256::ZERO; 256];
    for i in 1..256 {
        out[i] = keccak256(concat_fixed(out[i - 1].into(), out[i - 1].into()))
    }
    out
});

/// A [Page] is a portion of memory of size [PAGE_SIZE].
pub type Page = [u8; PAGE_SIZE];

/// A [CachedPage] is a [Page] with an in-memory cache of intermediate nodes.
pub struct CachedPage {
    data: Page,
    /// Storage for intermediate nodes
    cache: [[u8; 32]; PAGE_SIZE >> 5],
    /// Maps to true if the node is valid
    /// TODO(clabby): Use a bitmap / roaring bitmap
    valid: [bool; PAGE_SIZE >> 5],
}

impl Default for CachedPage {
    fn default() -> Self {
        Self {
            data: [0; PAGE_SIZE],
            cache: [[0; 32]; PAGE_SIZE >> 5],
            valid: [false; PAGE_SIZE >> 5],
        }
    }
}

impl CachedPage {
    pub fn invalidate(&mut self, page_addr: u32) {
        if page_addr >= PAGE_SIZE as u32 {
            panic!("Invalid page address");
        }

        // The first cache layer caches nodes that have two 32 byte leaf nodes.
        let mut key = ((1 << PAGE_ADDRESS_SIZE) | page_addr) >> 6;

        // SAFETY: mempirate no looping, me clock cycles
        // unsafe {
        //     let len = (31 - key.leading_zeros()) + 1;
        //     std::ptr::write_bytes(&mut self.valid[key as usize] as *mut bool, 0, len as usize);
        // }
        while key > 0 {
            self.valid[key as usize] = false;
            key >>= 1;
        }
    }

    pub fn invalidate_full(&mut self) {
        self.valid = [false; PAGE_SIZE >> 5];
    }

    pub fn merkle_root(&mut self) -> B256 {
        // First, hash the bottom layer.
        for i in (0..PAGE_SIZE).step_by(64) {
            let j = (PAGE_SIZE >> 6) + (i >> 6);
            if self.valid[j] {
                continue;
            }

            self.cache[j] = *keccak256(&self.data[i..i + 64]);
            self.valid[j] = true;
        }

        // Then, hash the cache layers.
        for i in (1..=(PAGE_SIZE >> 5) - 2).rev().step_by(2) {
            let j = i >> 1;
            if self.valid[j] {
                continue;
            }
            self.cache[j] = *keccak256(concat_fixed(self.cache[i], self.cache[i + 1]));
            self.valid[j] = true;
        }

        self.cache[1].into()
    }

    pub fn merklize_subtree(&mut self, g_index: usize) -> B256 {
        // Fill the cache by computing the merkle root.
        let _ = self.merkle_root();

        if g_index >= PAGE_SIZE >> 5 {
            if g_index >= (PAGE_SIZE >> 5) * 2 {
                panic!("Gindex too deep");
            }

            let node_index = g_index & (PAGE_ADDRESS_MASK >> 5);
            return B256::from_slice(&self.data[node_index << 5..(node_index << 5) + 32]);
        }

        self.cache[g_index].into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn cached_page_static() {
        let mut page = CachedPage::default();
        page.data[42] = 0xab;

        let g_index = ((1 << PAGE_ADDRESS_SIZE) | 42) >> 5;
        let node = page.merklize_subtree(g_index);
        let mut expected_leaf = B256::ZERO;
        expected_leaf[10] = 0xab;
        assert_eq!(node, expected_leaf, "Leaf nodes should not be hashed");

        let node = page.merklize_subtree(g_index >> 1);
        let expected_parent = keccak256(concat_fixed(ZERO_HASHES[0].into(), expected_leaf.into()));
        assert_eq!(node, expected_parent, "Parent should be correct");

        let node = page.merklize_subtree(g_index >> 2);
        let expected_grandparent =
            keccak256(concat_fixed(expected_parent.into(), ZERO_HASHES[1].into()));
        assert_eq!(node, expected_grandparent, "Grandparent should be correct");

        let pre = page.merkle_root();
        page.data[42] = 0xcd;
        let post = page.merkle_root();
        assert_eq!(
            pre, post,
            "Pre and post state should be equal until the cache is invalidated"
        );

        page.invalidate(42);
        let post_b = page.merkle_root();
        assert_ne!(
            post, post_b,
            "Pre and post state should be different after cache invalidation"
        );

        page.data[2000] = 0xef;
        page.invalidate(42);
        let post_c = page.merkle_root();
        assert_eq!(
            post_b, post_c,
            "Local invalidation is not global invalidation."
        );

        page.invalidate(2000);
        let post_d = page.merkle_root();
        assert_ne!(
            post_c, post_d,
            "Multiple invalidations should change the root."
        );

        page.data[1000] = 0xff;
        page.invalidate_full();
        let post_e = page.merkle_root();
        assert_ne!(
            post_d, post_e,
            "Full invalidation should always change the root."
        );
    }
}
