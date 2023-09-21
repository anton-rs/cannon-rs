//! This module contains all of the type aliases and enums used within this crate.

/// A [Page] is a portion of memory of size [crate::page::PAGE_SIZE].
pub type Page = [u8; crate::page::PAGE_SIZE];

/// A [StateWitness] is an encoded commitment to the current [crate::State] of the MIPS emulator.
pub type StateWitness = [u8; crate::witness::STATE_WITNESS_SIZE];

/// A [PageIndex] is the index of a [Page] within the [crate::Memory] mappings.
pub type PageIndex = u64;

/// A [Gindex] is a generalized index, defined as $2^{\text{depth}} + \text{index}$.
pub type Gindex = u64;

/// An [Address] is a 64 bit address in the MIPS emulator's memory.
pub type Address = u64;

/// The [VMStatus] is an indicator within the [StateWitness] hash that indicates
/// the current status of the MIPS emulator.
#[repr(u8)]
pub enum VMStatus {
    Valid = 0,
    Invalid = 1,
    Panic = 2,
    Unfinished = 3,
}

#[repr(u8)]
pub enum Fd {
    StdIn = 0,
    Stdout = 1,
    StdErr = 2,
    HintRead = 3,
    HintWrite = 4,
    PreimageRead = 5,
    PreimageWrite = 6,
}
