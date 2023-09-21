// #![doc = include_str!("../README.md")]
#![feature(generic_const_exprs)]
#![allow(incomplete_features, dead_code)]

mod memory;
pub use self::memory::{Address, Gindex, Memory, PageIndex};

mod page;
pub use self::page::{CachedPage, Page};

mod state;
pub use self::state::State;

mod traits;
pub use self::traits::StateWitnessHasher;

mod witness;
pub use self::witness::StateWitness;

mod utils;
