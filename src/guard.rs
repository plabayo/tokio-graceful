use std::{future::Future, mem::ManuallyDrop};

use crate::{
    sync::{Arc, AtomicUsize, JoinHandle, Ordering},
    trigger::{Receiver, Sender},
};

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
pub struct ShutdownGuard(ManuallyDrop<WeakShutdownGuard>);

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
        let value = ref_count.fetch_add(1, Ordering::SeqCst);
        tracing::trace!("new shutdown guard: ref_count+1: {}", value + 1);
        Self(ManuallyDrop::new(WeakShutdownGuard::new(
            trigger_rx, zero_tx, ref_count,
        )))
    }

    /// Returns a Future that gets fulfilled when cancellation (shutdown) is requested.
    ///
    /// The future will complete immediately if the token is already cancelled when this method is called.
    ///
    /// Use [`ShutdownGuard::cancelled_peek`] to check it once immediately without waiting.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe.
    ///
    /// # Panics
    ///
    /// This method panics if the internal mutex
    /// is poisoned while being used.
    #[inline]
    pub async fn cancelled(&self) {
        self.0.cancelled().await
    }

    /// Returns true in case the cancellation (shutdown) was right now already requested.
    ///
    /// Use [`ShutdownGuard::cancelled`] to wait for the cancellation (shutdown) to be requested.
    #[inline]
    pub fn cancelled_peek(&self) -> bool {
        self.0.cancelled_peek()
    }

    /// Returns a [`crate::sync::JoinHandle`] that can be awaited on
    /// to wait for the spawned task to complete. See
    /// [`crate::sync::spawn`] for more information.
    pub fn spawn_task<T>(&self, task: T) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let guard = self.clone();
        crate::sync::spawn(async move {
            let output = task.await;
            drop(guard);
            output
        })
    }

    /// Returns a Tokio [`crate::sync::JoinHandle`] that can be awaited on
    /// to wait for the spawned task (future) to complete. See
    /// [`crate::sync::spawn`] for more information.
    ///
    /// In contrast to [`ShutdownGuard::spawn_task`] this method consumes the guard,
    /// ensuring the guard is dropped once the task future is fulfilled.
    /// [`ShutdownGuard::spawn_task`]: crate::ShutdownGuard::spawn_task
    pub fn into_spawn_task<T>(self, task: T) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        crate::sync::spawn(async move {
            let output = task.await;
            drop(self);
            output
        })
    }

    /// Returns a Tokio [`crate::sync::JoinHandle`] that can be awaited on
    /// to wait for the spawned task (fn) to complete. See
    /// [`crate::sync::spawn`] for more information.
    pub fn spawn_task_fn<F, T>(&self, task: F) -> JoinHandle<T::Output>
    where
        F: FnOnce(ShutdownGuard) -> T + Send + 'static,
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let guard = self.clone();
        crate::sync::spawn(async move { task(guard).await })
    }

    /// Returns a Tokio [`crate::sync::JoinHandle`] that can be awaited on
    /// to wait for the spawned task (fn) to complete. See
    /// [`crate::sync::spawn`] for more information.
    ///
    /// In contrast to [`ShutdownGuard::spawn_task_fn`] this method consumes the guard,
    /// ensuring the guard is dropped once the task future is fulfilled.
    /// [`ShutdownGuard::spawn_task_fn`]: crate::ShutdownGuard::spawn_task_fn
    pub fn into_spawn_task_fn<F, T>(self, task: F) -> JoinHandle<T::Output>
    where
        F: FnOnce(ShutdownGuard) -> T + Send + 'static,
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        crate::sync::spawn(async move { task(self).await })
    }

    /// Downgrades the guard to a [`WeakShutdownGuard`],
    /// ensuring that the guard no longer prevents the
    /// [`Shutdown::shutdown`] future from completing.
    ///
    /// [`Shutdown::shutdown`]: crate::Shutdown::shutdown
    pub fn downgrade(mut self) -> WeakShutdownGuard {
        unsafe { ManuallyDrop::take(&mut self.0) }
    }

    /// Clones the guard as a [`WeakShutdownGuard`],
    /// ensuring that the cloned guard does not prevent the
    /// [`Shutdown::shutdown`] future from completing.
    ///
    /// [`Shutdown::shutdown`]: crate::Shutdown::shutdown
    pub fn clone_weak(&self) -> WeakShutdownGuard {
        ManuallyDrop::into_inner(self.0.clone())
    }
}

impl Clone for ShutdownGuard {
    fn clone(&self) -> Self {
        let value = &self.0.ref_count.fetch_add(1, Ordering::SeqCst);
        tracing::trace!("clone shutdown guard: ref_count+1: {}", value + 1);
        Self(self.0.clone())
    }
}

impl From<WeakShutdownGuard> for ShutdownGuard {
    fn from(weak_guard: WeakShutdownGuard) -> ShutdownGuard {
        let value = weak_guard.ref_count.fetch_add(1, Ordering::SeqCst);
        tracing::trace!("from weak shutdown guard: ref_count+1: {}", value + 1);
        Self(ManuallyDrop::new(weak_guard))
    }
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        let cnt = self.0.ref_count.fetch_sub(1, Ordering::SeqCst);
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
    /// Use [`WeakShutdownGuard::cancelled_peek`] to check it once immediately without waiting.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe.
    ///
    /// # Panics
    ///
    /// This method panics if the internal mutex
    /// is poisoned while being used.
    #[inline]
    pub async fn cancelled(&self) {
        self.trigger_rx.clone().await;
    }

    /// Returns true in case the cancellation (shutdown) was right now already requested.
    ///
    /// Use [`WeakShutdownGuard::cancelled`] to wait for the cancellation (shutdown) to be requested.
    #[inline]
    pub fn cancelled_peek(&self) -> bool {
        self.trigger_rx.closed()
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
    /// This method panics if the internal mutex
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
