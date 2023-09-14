#[cfg(loom)]
mod loom;
#[cfg(loom)]
pub use self::loom::*;

#[cfg(not(loom))]
mod default;
#[cfg(not(loom))]
pub use default::*;

pub use tokio::task::{spawn, JoinHandle};
