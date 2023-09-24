//! This module contains utility and helper functions for this crate.

use alloy_primitives::{keccak256, B256};

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
pub(crate) fn keccak_concat_fixed<T, const N: usize, const M: usize>(a: [T; N], b: [T; M]) -> B256
where
    T: Copy + Default,
    [T; N + M]: AsRef<[u8]>,
{
    keccak256(concat_fixed(a, b))
}
