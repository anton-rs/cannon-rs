#![allow(unused_imports)]

/// Performs a tracing debug if the `tracing` feature is enabled.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::debug!($($arg)*);
    };
}
pub use debug;

/// Performs a tracing info if the `tracing` feature is enabled.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::info!($($arg)*);
    };
}
pub use info;

/// Performs a tracing error if the `tracing` feature is enabled.
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::error!($($arg)*);
    };
}
pub use error;

/// Performs a tracing warn if the `tracing` feature is enabled.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::warn!($($arg)*);
    };
}
pub use crate::warn;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug() {
        debug!("test");
    }
}
