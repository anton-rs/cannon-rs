#![doc = include_str!("../README.md")]

pub(crate) mod traces;

mod oracle;
pub use oracle::{OracleClient, OracleServer};

mod traits;
pub use traits::{FileChannel, Hint, Hinter, Key, Oracle};

mod types;
pub use types::{Keccak256Key, KeyType, LocalIndexKey, PreimageGetter};

mod hints;
pub use hints::{HintReader, HintWriter};

mod file_chan;
pub use file_chan::{create_bidirectional_channel, ReadWritePair};
