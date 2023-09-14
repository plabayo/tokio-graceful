#[cfg(loom)]
pub use loom::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex, Ordering,
};

#[cfg(not(loom))]
pub use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
