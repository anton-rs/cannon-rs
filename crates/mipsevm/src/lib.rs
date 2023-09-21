// #![doc = include_str!("../README.md")]
#![feature(generic_const_exprs)]
#![allow(incomplete_features, dead_code)]

mod memory;
pub use self::memory::Memory;

mod page;
pub use self::page::CachedPage;

mod state;
pub use self::state::State;

mod traits;
pub use self::traits::{PreimageOracle, StateWitnessHasher};

mod witness;

mod utils;

mod types;
pub use types::{Address, Fd, Gindex, Page, PageIndex, StateWitness, VMStatus};

mod mips;
pub use mips::InstrumentedState;
