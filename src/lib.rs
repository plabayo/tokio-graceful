#![doc = include_str!("../README.md")]

mod guard;
pub use guard::{ShutdownGuard, WeakShutdownGuard};

mod shutdown;
pub use shutdown::Shutdown;
