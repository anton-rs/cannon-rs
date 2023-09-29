//! This module contains all of the type aliases and enums used within this crate.

use crate::{page::PAGE_SIZE, CachedPage};
use serde::{
    de::{self, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{cell::RefCell, rc::Rc};

/// A [Page] is a portion of memory of size `PAGE_SIZE`.
pub type Page = [u8; crate::page::PAGE_SIZE];

/// A wrapper around the [Page] type for serialization and deserialization.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PageWrapper(pub Page);

/// A wrapper around a shared [CachedPage] type for serialization and deserialization.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct SharedCachedPageWrapper(pub Rc<RefCell<CachedPage>>);

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

impl Serialize for PageWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for element in self.0 {
            seq.serialize_element(&element)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for PageWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PageVisitor;

        impl<'de> Visitor<'de> for PageVisitor {
            type Value = PageWrapper;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a byte array")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<PageWrapper, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let mut page = [0u8; PAGE_SIZE];
                for (i, page) in page.iter_mut().enumerate().take(PAGE_SIZE) {
                    *page = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(PageWrapper(page))
            }
        }

        deserializer.deserialize_seq(PageVisitor)
    }
}

impl Serialize for SharedCachedPageWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Borrow the value immutably
        let value_ref = self.0.borrow();
        // Serialize the inner value
        value_ref.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SharedCachedPageWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize the inner value
        let value = CachedPage::deserialize(deserializer)?;
        // Wrap it in Rc<RefCell<CachedPage>> and return
        Ok(SharedCachedPageWrapper(Rc::new(RefCell::new(value))))
    }
}
