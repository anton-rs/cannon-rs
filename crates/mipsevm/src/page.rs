//! This module contains the data structure for a [Page] within the MIPS emulator's [Memory].

use crate::{utils::keccak_concat_hashes, Address, Gindex, Page};
use anyhow::Result;
use once_cell::sync::Lazy;

#[cfg(not(feature = "simd-keccak"))]
use crate::utils::keccak256;

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
        out[i] = *keccak_concat_hashes(out[i - 1], out[i - 1])
    }
    out
});

/// Precomputed cache of a merkleized page with all zero data.
pub(crate) static DEFAULT_CACHE: Lazy<[[u8; 32]; PAGE_SIZE_WORDS]> = Lazy::new(|| {
    let mut page = CachedPage {
        data: [0; PAGE_SIZE],
        cache: [[0; 32]; PAGE_SIZE_WORDS],
        valid: [false; PAGE_SIZE / 32],
    };
    page.merkle_root().unwrap();
    page.cache
});

/// A [CachedPage] is a [Page] with an in-memory cache of intermediate nodes.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CachedPage {
    pub data: Page,
    /// Storage for intermediate nodes
    pub cache: [[u8; 32]; PAGE_SIZE_WORDS],
    /// Bitmap for 128 nodes. 1 if valid, 0 if invalid.
    pub valid: [bool; PAGE_SIZE / 32],
}

impl Default for CachedPage {
    fn default() -> Self {
        Self {
            data: [0; PAGE_SIZE],
            cache: *DEFAULT_CACHE,
            valid: [true; PAGE_SIZE / 32],
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
    #[inline(always)]
    pub fn invalidate(&mut self, page_addr: Address) -> Result<()> {
        if page_addr >= PAGE_SIZE as Address {
            anyhow::bail!("Invalid page address: {}", page_addr);
        }

        // The first cache layer caches nodes that have two 32 byte leaf nodes.
        let key = ((1 << PAGE_ADDRESS_SIZE) | page_addr) >> 6;

        // Invalidate the key and all subsequent keys using slicing
        self.valid[..=key as usize].fill(false);

        Ok(())
    }

    /// Invalidate the entire [Page].
    ///
    /// This is equivalent to calling `invalidate` on every address in the page.
    #[inline(always)]
    pub fn invalidate_full(&mut self) {
        self.valid = [false; PAGE_SIZE / 32];
    }

    /// Compute the merkle root of the [Page].
    ///
    /// ## Returns
    /// - The 32 byte merkle root hash of the [Page].
    #[inline(always)]
    pub fn merkle_root(&mut self) -> Result<[u8; 32]> {
        self.merkleize_subtree(1)
    }

    /// Compute the merkle root for the subtree rooted at the given generalized index.
    ///
    /// ### Takes
    /// - `g_index`: The generalized index of the subtree to merkleize.
    ///
    /// ### Returns
    /// - A [Result] containing the 32 byte merkle root hash of the subtree or an error if the
    ///  generalized index is too deep.
    #[inline(always)]
    pub fn merkleize_subtree(&mut self, g_index: Gindex) -> Result<[u8; 32]> {
        // Cast to usize to avoid `as usize` everywhere.
        let g_index = g_index as usize;

        if (PAGE_SIZE_WORDS..PAGE_SIZE_WORDS * 2).contains(&g_index) {
            let node_index = (g_index & (PAGE_ADDRESS_MASK >> 5)) << 5;
            return Ok(self.data[node_index..node_index + 32].try_into()?);
        } else if g_index >= PAGE_SIZE_WORDS * 2 {
            anyhow::bail!("Generalized index is too deep: {}", g_index);
        } else if self.valid[g_index] {
            return Ok(self.cache[g_index]);
        }

        let hash = if g_index >= PAGE_SIZE_WORDS >> 1 {
            // This is a leaf node.
            let data_idx = (g_index - (PAGE_SIZE_WORDS >> 1)) << 6;
            #[cfg(feature = "simd-keccak")]
            {
                let mut out = [0u8; 32];
                keccak256_aarch64_simd::simd_keccak256_64b_single(
                    &self.data[data_idx..data_idx + 64],
                    &mut out,
                );
                out
            }

            #[cfg(not(feature = "simd-keccak"))]
            *keccak256(&self.data[data_idx..data_idx + 64])
        } else {
            // This is an internal node.
            let left_child = g_index << 1;
            let right_child = left_child + 1;

            // Ensure children are hashed.
            *keccak_concat_hashes(
                self.merkleize_subtree(left_child as Gindex)?,
                self.merkleize_subtree(right_child as Gindex)?,
            )
        };
        self.valid[g_index] = true;
        self.cache[g_index] = hash;
        Ok(hash)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn cached_page_static() {
        let mut page = CachedPage::default();
        page.data[42] = 0xab;
        page.invalidate(42).unwrap();

        let g_index = ((1 << PAGE_ADDRESS_SIZE) | 42) >> 5;
        let node = page.merkleize_subtree(g_index).unwrap();
        let mut expected_leaf = [0u8; 32];
        expected_leaf[10] = 0xab;
        assert_eq!(node, expected_leaf, "Leaf nodes should not be hashed");

        let node = page.merkleize_subtree(g_index >> 1).unwrap();
        let expected_parent = keccak_concat_hashes(ZERO_HASHES[0].into(), expected_leaf.into());
        assert_eq!(node, expected_parent, "Parent should be correct");

        let node = page.merkleize_subtree(g_index >> 2).unwrap();
        let expected_grandparent =
            keccak_concat_hashes(expected_parent.into(), ZERO_HASHES[1].into());
        assert_eq!(node, expected_grandparent, "Grandparent should be correct");

        let pre = page.merkle_root().unwrap();
        page.data[42] = 0xcd;
        let post = page.merkle_root().unwrap();
        assert_eq!(
            pre, post,
            "Pre and post state should be equal until the cache is invalidated"
        );

        page.invalidate(42).unwrap();
        let post_b = page.merkle_root().unwrap();
        assert_ne!(
            post, post_b,
            "Pre and post state should be different after cache invalidation"
        );

        page.data[2000] = 0xef;
        page.invalidate(42).unwrap();
        let post_c = page.merkle_root().unwrap();
        assert_eq!(
            post_b, post_c,
            "Local invalidation is not global invalidation."
        );

        page.invalidate(2000).unwrap();
        let post_d = page.merkle_root().unwrap();
        assert_ne!(
            post_c, post_d,
            "Multiple invalidations should change the root."
        );

        page.data[1000] = 0xff;
        page.invalidate_full();
        let post_e = page.merkle_root().unwrap();
        assert_ne!(
            post_d, post_e,
            "Full invalidation should always change the root."
        );
    }
}
