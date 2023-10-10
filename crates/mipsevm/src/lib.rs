// #![doc = include_str!("../README.md")]
#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

pub(crate) mod traces;

mod memory;
pub use self::memory::Memory;

mod page;
pub use self::page::CachedPage;

mod state;
pub use self::state::State;

mod traits;
pub use self::traits::{PreimageOracle, StateWitnessHasher};

mod witness;
pub use witness::{StepWitness, STATE_WITNESS_SIZE};

mod utils;

mod types;
pub use types::{Address, Fd, Gindex, Page, PageIndex, StateWitness, VMStatus};

mod mips;
pub use mips::InstrumentedState;

mod patch;
pub use patch::{load_elf, patch_go, patch_stack, MultiReader};

pub mod ser;

pub mod test_utils;
