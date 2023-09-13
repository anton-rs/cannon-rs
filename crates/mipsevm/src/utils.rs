//! Contains utility and helper functions for the emulator.

/// Concatenate two fixed sized arrays together into a new array with minimal reallocation.
#[inline(always)]
pub(crate) fn concat_arrays<T, const N: usize, const M: usize>(a: [T; N], b: [T; M]) -> [T; N + M]
where
    T: Copy + Default,
{
    let mut concatenated: [T; N + M] = [T::default(); N + M];
    let (left, right) = concatenated.split_at_mut(N);
    left.copy_from_slice(&a);
    right.copy_from_slice(&b);
    concatenated
}
