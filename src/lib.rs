mod guard;
pub use guard::{ShutdownGuard, WeakShutdownGuard};

mod shutdown;
pub use shutdown::Shutdown;
