//! This module contains the data structure for a [Page] within the MIPS emulator's [Memory].

use crate::{utils::keccak_concat_fixed, Address, Gindex, Page};
use alloy_primitives::keccak256;
use anyhow::Result;
use once_cell::sync::Lazy;

pub(crate) const PAGE_ADDRESS_SIZE: usize = 12;
pub(crate) const PAGE_KEY_SIZE: usize = 32 - PAGE_ADDRESS_SIZE;
pub(crate) const PAGE_SIZE: usize = 1 << PAGE_ADDRESS_SIZE;
pub(crate) const PAGE_SIZE_WORDS: usize = PAGE_SIZE >> 5;
pub(crate) const PAGE_ADDRESS_MASK: usize = PAGE_SIZE - 1;
pub(crate) const MAX_PAGE_COUNT: usize = 1 << PAGE_KEY_SIZE;
pub(crate) const PAGE_KEY_MASK: usize = MAX_PAGE_COUNT - 1;

/// Precomputed hashes of each full-zero range sub-tree level.
pub(crate) static ZERO_HASHES: Lazy<[[u8; 32]; 256]> = Lazy::new(|| {
    let mut out = [[0u8; 32]; 256];
    for i in 1..256 {
        out[i] = *keccak_concat_fixed(out[i - 1], out[i - 1])
    }
    out
});

/// A [CachedPage] is a [Page] with an in-memory cache of intermediate nodes.
#[derive(Debug, Clone, Copy)]
pub struct CachedPage {
    pub data: Page,
    /// Storage for intermediate nodes
    pub cache: [[u8; 32]; PAGE_SIZE_WORDS],
    /// Bitmap for 128 nodes. 1 if valid, 0 if invalid.
    valid: u128,
}

impl Default for CachedPage {
    fn default() -> Self {
        Self {
            data: [0; PAGE_SIZE],
            cache: [[0; 32]; PAGE_SIZE_WORDS],
            valid: 0,
        }
    }
}

impl CachedPage {
    /// Invalidate a given address within the [Page].
    ///
    /// ### Takes
    /// - `page_addr`: The [Address] to invalidate within the [Page].
    ///
    /// ### Returns
    /// - A [Result] indicating if the operation was successful.
    pub fn invalidate(&mut self, page_addr: Address) -> Result<()> {
        if page_addr >= PAGE_SIZE as Address {
            anyhow::bail!("Invalid page address: {}", page_addr);
        }

        // The first cache layer caches nodes that have two 32 byte leaf nodes.
        let key = ((1 << PAGE_ADDRESS_SIZE) | page_addr) >> 6;

        // Create a mask where all bits from position `127 - key` and above are set
        let mask: u128 = !((1 << (127 - key)) - 1);

        // Apply the mask to the valid bitmap
        self.valid &= !mask;

        Ok(())
    }

    /// Invalidate the entire [Page].
    ///
    /// This is equivalent to calling `invalidate` on every address in the page.
    pub fn invalidate_full(&mut self) {
        self.valid = 0;
    }

    /// Compute the merkle root of the [Page].
    ///
    /// ## Returns
    /// - The 32 byte merkle root hash of the [Page].
    pub fn merkle_root(&mut self) -> [u8; 32] {
        // First, hash the bottom layer.
        for i in (0..PAGE_SIZE).step_by(64) {
            let j = (PAGE_SIZE_WORDS >> 1) + (i >> 6);
            if self.is_valid(j) {
                continue;
            }

            self.cache[j] = *keccak256(&self.data[i..i + 64]);
            self.set_valid(j, true);
        }

        // Then, hash the cache layers.
        for i in (1..=PAGE_SIZE_WORDS - 2).rev().step_by(2) {
            let j = i >> 1;
            if self.is_valid(j) {
                continue;
            }
            let (a, b) = self.cache.split_at_mut(i);
            a[j] = *keccak_concat_fixed(b[0], b[1]);
            self.set_valid(j, true);
        }

        self.cache[1]
    }

    /// Fill the cache with the merkleized subtree who's root is the passed generalized index.
    ///
    /// ### Takes
    /// - `g_index`: The generalized index of the subtree to merkleize.
    ///
    /// ### Returns
    /// - The 32 byte merkle root hash of the subtree.
    fn fill_cache(&mut self, g_index: usize) -> [u8; 32] {
        if self.is_valid(g_index) {
            return self.cache[g_index];
        }

        let hash = if g_index >= PAGE_SIZE_WORDS >> 1 {
            // This is a leaf node.
            let data_idx = (g_index - (PAGE_SIZE_WORDS >> 1)) << 6;
            *keccak256(&self.data[data_idx..data_idx + 64])
        } else {
            // This is an internal node.
            let left_child = g_index << 1;
            let right_child = left_child + 1;

            // Ensure children are hashed.
            self.fill_cache(left_child);
            self.fill_cache(right_child);

            *keccak_concat_fixed(self.cache[left_child], self.cache[right_child])
        };
        self.set_valid(g_index, true);
        self.cache[g_index] = hash;
        hash
    }

    /// Compute the merkle root for the subtree rooted at the given generalized index.
    ///
    /// ### Takes
    /// - `g_index`: The generalized index of the subtree to merkleize.
    ///
    /// ### Returns
    /// - A [Result] containing the 32 byte merkle root hash of the subtree or an error if the
    ///  generalized index is too deep.
    pub fn merkleize_subtree(&mut self, g_index: Gindex) -> Result<[u8; 32]> {
        if (PAGE_SIZE_WORDS..PAGE_SIZE_WORDS * 2).contains(&(g_index as usize)) {
            let node_index = g_index as usize & (PAGE_ADDRESS_MASK >> 5);
            let start = node_index << 5;
            return Ok(self.data[start..start + 32].try_into()?);
        } else if g_index as usize >= PAGE_SIZE_WORDS * 2 {
            anyhow::bail!("Generalized index is too deep: {}", g_index);
        }

        Ok(self.fill_cache(g_index as usize))
    }

    /// Check if a key is valid within the bitmap.
    ///
    /// ### Takes
    /// - `key`: The key to check.
    #[inline(always)]
    pub fn is_valid(&self, key: usize) -> bool {
        let flag = 1 << (127 - key);
        self.valid & flag == flag
    }

    /// Set a key as valid or invalid within the bitmap.
    ///
    /// ### Takes
    /// - `key`: The key to set.
    /// - `valid`: Whether the key should be set as valid or invalid.
    #[inline(always)]
    pub fn set_valid(&mut self, key: usize, valid: bool) {
        let flag_offset = 127 - key;
        self.valid &= !(1 << flag_offset);
        self.valid |= (valid as u128) << flag_offset;
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
        let node = page.merkleize_subtree(g_index).unwrap();
        let mut expected_leaf = [0u8; 32];
        expected_leaf[10] = 0xab;
        assert_eq!(node, expected_leaf, "Leaf nodes should not be hashed");

        let node = page.merkleize_subtree(g_index >> 1).unwrap();
        let expected_parent = keccak_concat_fixed(ZERO_HASHES[0].into(), expected_leaf.into());
        assert_eq!(node, expected_parent, "Parent should be correct");

        let node = page.merkleize_subtree(g_index >> 2).unwrap();
        let expected_grandparent =
            keccak_concat_fixed(expected_parent.into(), ZERO_HASHES[1].into());
        assert_eq!(node, expected_grandparent, "Grandparent should be correct");

        let pre = page.merkle_root();
        page.data[42] = 0xcd;
        let post = page.merkle_root();
        assert_eq!(
            pre, post,
            "Pre and post state should be equal until the cache is invalidated"
        );

        page.invalidate(42).unwrap();
        let post_b = page.merkle_root();
        assert_ne!(
            post, post_b,
            "Pre and post state should be different after cache invalidation"
        );

        page.data[2000] = 0xef;
        page.invalidate(42).unwrap();
        let post_c = page.merkle_root();
        assert_eq!(
            post_b, post_c,
            "Local invalidation is not global invalidation."
        );

        page.invalidate(2000).unwrap();
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
