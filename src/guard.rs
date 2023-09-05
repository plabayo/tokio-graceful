use std::{
    future::Future,
    sync::{atomic::AtomicUsize, Arc},
};

use tokio::task::JoinHandle;

use crate::trigger::{Receiver, Sender};

/// A guard, linked to a [`Shutdown`] struct,
/// prevents the [`Shutdown::shutdown`] future from completing.
///
/// Can be cloned to create multiple [`ShutdownGuard`]s
/// and can be downgraded to a [`WeakShutdownGuard`] to
/// no longer prevent the [`Shutdown::shutdown`] future from completing.
///
/// [`Shutdown`]: crate::Shutdown
/// [`Shutdown::shutdown`]: crate::Shutdown::shutdown
#[derive(Debug)]
pub struct ShutdownGuard(WeakShutdownGuard);

/// A weak guard, linked to a [`Shutdown`] struct,
/// is similar to a [`ShutdownGuard`] but does not
/// prevent the [`Shutdown::shutdown`] future from completing.
///
/// [`Shutdown`]: crate::Shutdown
/// [`Shutdown::shutdown`]: crate::Shutdown::shutdown
#[derive(Debug, Clone)]
pub struct WeakShutdownGuard {
    pub(crate) trigger_rx: Receiver,
    pub(crate) zero_tx: Sender,
    pub(crate) ref_count: Arc<AtomicUsize>,
}

impl ShutdownGuard {
    pub(crate) fn new(trigger_rx: Receiver, zero_tx: Sender, ref_count: Arc<AtomicUsize>) -> Self {
        let value = ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tracing::trace!("new shutdown guard: ref_count+1: {}", value + 1);
        Self(WeakShutdownGuard::new(trigger_rx, zero_tx, ref_count))
    }

    /// Returns a Future that gets fulfilled when cancellation (shutdown) is requested.
    ///
    /// The future will complete immediately if the token is already cancelled when this method is called.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe.
    ///
    /// # Panics
    ///
    /// This method panics if the iternal mutex
    /// is poisoned while being used.
    #[inline]
    pub async fn cancelled(&self) {
        self.0.cancelled().await
    }

    /// Returns a Tokio [`JoinHandle`] that can be awaited on
    /// to wait for the spawned task to complete. See
    /// [`tokio::spawn`] for more information.
    ///
    /// [`JoinHandle`]: https://docs.rs/tokio/*/tokio/task/struct.JoinHandle.html
    /// [`tokio::spawn`]: https://docs.rs/tokio/*/tokio/task/fn.spawn.html
    pub fn spawn_task<T>(&self, task: T) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let guard = self.clone();
        tokio::spawn(async move {
            let output = task.await;
            drop(guard);
            output
        })
    }

    /// Returns a Tokio [`JoinHandle`] that can be awaited on
    /// to wait for the spawned task (future) to complete. See
    /// [`tokio::spawn`] for more information.
    ///
    /// In contrast to [`ShutdownGuard::spawn_task`] this method consumes the guard,
    /// ensuring the guard is dropped once the task future is fulfilled.
    ///
    /// [`JoinHandle`]: https://docs.rs/tokio/*/tokio/task/struct.JoinHandle.html
    /// [`tokio::spawn`]: https://docs.rs/tokio/*/tokio/task/fn.spawn.html
    /// [`ShutdownGuard::spawn_task`]: crate::ShutdownGuard::spawn_task
    pub fn into_spawn_task<T>(self, task: T) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        tokio::spawn(async move {
            let output = task.await;
            drop(self);
            output
        })
    }

    /// Returns a Tokio [`JoinHandle`] that can be awaited on
    /// to wait for the spawned task (fn) to complete. See
    /// [`tokio::spawn`] for more information.
    ///
    /// [`JoinHandle`]: https://docs.rs/tokio/*/tokio/task/struct.JoinHandle.html
    /// [`tokio::spawn`]: https://docs.rs/tokio/*/tokio/task/fn.spawn.html
    pub fn spawn_task_fn<F, T>(&self, task: F) -> JoinHandle<T::Output>
    where
        F: FnOnce(ShutdownGuard) -> T + Send + 'static,
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let guard = self.clone();
        tokio::spawn(async move { task(guard).await })
    }

    /// Returns a Tokio [`JoinHandle`] that can be awaited on
    /// to wait for the spawned task (fn) to complete. See
    /// [`tokio::spawn`] for more information.
    ///
    /// In contrast to [`ShutdownGuard::spawn_task_fn`] this method consumes the guard,
    /// ensuring the guard is dropped once the task future is fulfilled.
    ///
    /// [`JoinHandle`]: https://docs.rs/tokio/*/tokio/task/struct.JoinHandle.html
    /// [`tokio::spawn`]: https://docs.rs/tokio/*/tokio/task/fn.spawn.html
    /// [`ShutdownGuard::spawn_task_fn`]: crate::ShutdownGuard::spawn_task_fn
    pub fn into_spawn_task_fn<F, T>(self, task: F) -> JoinHandle<T::Output>
    where
        F: FnOnce(ShutdownGuard) -> T + Send + 'static,
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        tokio::spawn(async move { task(self).await })
    }

    /// Downgrades the guard to a [`WeakShutdownGuard`],
    /// ensuring that the guard no longer prevents the
    /// [`Shutdown::shutdown`] future from completing.
    ///
    /// [`Shutdown::shutdown`]: crate::Shutdown::shutdown
    pub fn downgrade(self) -> WeakShutdownGuard {
        self.0
    }

    /// Clones the guard as a [`WeakShutdownGuard`],
    /// ensuring that the cloned guard does not prevent the
    /// [`Shutdown::shutdown`] future from completing.
    ///
    /// [`Shutdown::shutdown`]: crate::Shutdown::shutdown
    pub fn clone_weak(&self) -> WeakShutdownGuard {
        self.0.clone()
    }
}

impl Clone for ShutdownGuard {
    fn clone(&self) -> Self {
        let value = &self
            .0
            .ref_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tracing::trace!("clone shutdown guard: ref_count+1: {}", value + 1);
        Self(self.0.clone())
    }
}

impl From<WeakShutdownGuard> for ShutdownGuard {
    fn from(weak_guard: WeakShutdownGuard) -> ShutdownGuard {
        let value = weak_guard
            .ref_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tracing::trace!("from weak shutdown guard: ref_count+1: {}", value + 1);
        Self(weak_guard)
    }
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        let cnt = self
            .0
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        tracing::trace!("drop shutdown guard: ref_count-1: {}", cnt - 1);
        if cnt == 1 {
            self.0.zero_tx.trigger();
        }
    }
}

impl WeakShutdownGuard {
    pub(crate) fn new(trigger_rx: Receiver, zero_tx: Sender, ref_count: Arc<AtomicUsize>) -> Self {
        Self {
            trigger_rx,
            zero_tx,
            ref_count,
        }
    }

    /// Returns a Future that gets fulfilled when cancellation (shutdown) is requested.
    ///
    /// The future will complete immediately if the token is already cancelled when this method is called.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe.
    ///
    /// # Panics
    ///
    /// This method panics if the iternal mutex
    /// is poisoned while being used.
    #[inline]
    pub async fn cancelled(&self) {
        self.trigger_rx.clone().await;
    }

    /// Returns a Future that gets fulfilled when cancellation (shutdown) is requested.
    ///
    /// In contrast to [`ShutdownGuard::cancelled`] this method consumes the guard,
    /// ensuring the guard is dropped once the future is fulfilled.
    ///
    /// The future will complete immediately if the token is already cancelled when this method is called.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe.
    ///
    /// # Panics
    ///
    /// This method panics if the iternal mutex
    /// is poisoned while being used.
    #[inline]
    pub async fn into_cancelled(self) {
        self.cancelled().await;
    }

    /// Upgrades the weak guard to a [`ShutdownGuard`],
    /// ensuring that the guard has to be dropped prior to
    /// being able to complete the [`Shutdown::shutdown`] future.
    ///
    /// [`Shutdown::shutdown`]: crate::Shutdown::shutdown
    #[inline]
    pub fn upgrade(self) -> ShutdownGuard {
        self.into()
    }
}
