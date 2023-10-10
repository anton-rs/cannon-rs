//! This module contains all of the type aliases and enums used within this crate.

use crate::CachedPage;
use std::{cell::RefCell, rc::Rc};

/// A [Page] is a portion of memory of size `PAGE_SIZE`.
pub type Page = [u8; crate::page::PAGE_SIZE];

/// A [CachedPage] with shared ownership.
pub type SharedCachedPage = Rc<RefCell<CachedPage>>;

/// A [StateWitness] is an encoded commitment to the current [crate::State] of the MIPS emulator.
pub type StateWitness = [u8; crate::witness::STATE_WITNESS_SIZE];

/// A [PageIndex] is the index of a [Page] within the [crate::Memory] mappings.
pub type PageIndex = u64;

/// A [Gindex] is a generalized index, defined as $2^{\text{depth}} + \text{index}$.
pub type Gindex = u64;

/// An [Address] is a 32 bit address in the MIPS emulator's memory.
pub type Address = u32;

/// The [VMStatus] is an indicator within the [StateWitness] hash that indicates
/// the current status of the MIPS emulator.
#[repr(u8)]
pub enum VMStatus {
    Valid = 0,
    Invalid = 1,
    Panic = 2,
    Unfinished = 3,
}

/// Identifiers for special file descriptors used by the MIPS emulator.
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

impl TryFrom<u8> for Fd {
    type Error = anyhow::Error;

    fn try_from(n: u8) -> Result<Self, Self::Error> {
        match n {
            0 => Ok(Fd::StdIn),
            1 => Ok(Fd::Stdout),
            2 => Ok(Fd::StdErr),
            3 => Ok(Fd::HintRead),
            4 => Ok(Fd::HintWrite),
            5 => Ok(Fd::PreimageRead),
            6 => Ok(Fd::PreimageWrite),
            _ => anyhow::bail!("Failed to convert {} to Fd", n),
        }
    }
}

/// A [Syscall] is a system call that can be made within the MIPS emulator.
pub enum Syscall {
    Mmap = 4090,
    Brk = 4045,
    Clone = 4120,
    ExitGroup = 4246,
    Read = 4003,
    Write = 4004,
    Fcntl = 4055,
}

impl TryFrom<u32> for Syscall {
    type Error = anyhow::Error;

    fn try_from(n: u32) -> Result<Self, Self::Error> {
        match n {
            4090 => Ok(Syscall::Mmap),
            4045 => Ok(Syscall::Brk),
            4120 => Ok(Syscall::Clone),
            4246 => Ok(Syscall::ExitGroup),
            4003 => Ok(Syscall::Read),
            4004 => Ok(Syscall::Write),
            4055 => Ok(Syscall::Fcntl),
            _ => anyhow::bail!("Failed to convert {} to Syscall", n),
        }
    }
}
