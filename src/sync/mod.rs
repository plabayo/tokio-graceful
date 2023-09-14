#[cfg(loom)]
mod loom;
#[cfg(loom)]
pub use loom::*;

#[cfg(not(loom))]
mod default;
#[cfg(not(loom))]
pub use default::*;
