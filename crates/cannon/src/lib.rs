#![doc = include_str!("../README.md")]

mod builder;
pub use builder::KernelBuilder;

mod kernel;
pub use kernel::Kernel;

pub mod compressor;
