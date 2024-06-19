//! This module contains utility and helper functions for this crate.

use alloy_primitives::B256;

/// Concatenate two fixed sized arrays together into a new array with minimal reallocation.
#[inline(always)]
pub(crate) fn concat_fixed<T, const N: usize, const M: usize>(a: [T; N], b: [T; M]) -> [T; N + M]
where
    T: Copy + Default,
{
    let mut concatenated: [T; N + M] = [T::default(); N + M];
    let (left, right) = concatenated.split_at_mut(N);
    left.copy_from_slice(&a);
    right.copy_from_slice(&b);
    concatenated
}

/// Hash the concatenation of two 32 byte digests.
#[inline(always)]
pub(crate) fn keccak_concat_hashes(a: [u8; 32], b: [u8; 32]) -> B256 {
    #[cfg(feature = "simd-keccak")]
    {
        let mut out = B256::ZERO;
        keccak256_aarch64_simd::simd_keccak256_64b_single(&concat_fixed(a, b), out.as_mut());
        out
    }

    #[cfg(not(feature = "simd-keccak"))]
    keccak256(concat_fixed(a, b).as_slice())
}

#[inline(always)]
pub(crate) fn keccak256<T: AsRef<[u8]>>(input: T) -> B256 {
    let mut out = B256::ZERO;
    xkcp_rs::keccak256(input.as_ref(), out.as_mut());
    out
}

/// Perform a sign extension of a value embedded in the lower bits of `data` up to
/// the `index`th bit.
///
/// ### Takes
/// - `data`: The data to sign extend.
/// - `index`: The index of the bit to sign extend to.
///
/// ### Returns
/// - The sign extended value.
#[inline(always)]
pub(crate) fn sign_extend(data: u64, index: u64) -> u64 {
    let is_signed = (data >> (index - 1)) != 0;
    let signed = ((1 << (64 - index)) - 1) << index;
    let mask = (1 << index) - 1;
    if is_signed {
        (data & mask) | signed
    } else {
        data & mask
    }
}
