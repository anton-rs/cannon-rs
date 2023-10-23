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

/// Hash the concatenation of two fixed sized arrays.
#[inline(always)]
pub(crate) fn keccak_concat_fixed<const N: usize, const M: usize>(a: [u8; N], b: [u8; M]) -> B256
where
    [(); N + M]:,
{
    keccak256(concat_fixed(a, b).as_slice())
}

#[inline(always)]
pub(crate) fn keccak256<T: AsRef<[u8]>>(input: T) -> B256 {
    let mut out = B256::ZERO;
    xkcp_rs::keccak256(input.as_ref(), out.as_mut());
    out
}
