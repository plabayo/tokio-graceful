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
    pub(crate) shutdown_signal_trigger_rx: Option<Receiver>,
    pub(crate) zero_tx: Sender,
    pub(crate) ref_count: Arc<AtomicUsize>,
}

impl ShutdownGuard {
    pub(crate) fn new(
        trigger_rx: Receiver,
        shutdown_signal_trigger_rx: Option<Receiver>,
        zero_tx: Sender,
        ref_count: Arc<AtomicUsize>,
    ) -> Self {
        let value = ref_count.fetch_add(1, Ordering::SeqCst);
        tracing::trace!("new shutdown guard: ref_count+1: {}", value + 1);
        Self(ManuallyDrop::new(WeakShutdownGuard::new(
            trigger_rx,
            shutdown_signal_trigger_rx,
            zero_tx,
            ref_count,
        )))
    }

    /// Returns a Future that gets fulfilled when cancellation (shutdown) is requested
    /// and the delay (if any) duration has been awaited.
    ///
    /// Use [`Self::shutdown_signal_triggered`] for tasks that do not
    /// require this opt-in delay buffer duration.
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

    /// Returns a Future that gets fulfilled when cancellation (shutdown) is requested.
    ///
    /// Use [`Self::cancelled`] if you want to make sure the future
    /// only completes when the buffer delay has been awaited.
    ///
    /// In case no delay has been configured for the parent `Shutdown`,
    /// this function will be equal in behaviour to [`Self::cancelled`].
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
    pub async fn shutdown_signal_triggered(&self) {
        self.0.shutdown_signal_triggered().await
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
    pub(crate) fn new(
        trigger_rx: Receiver,
        shutdown_signal_trigger_rx: Option<Receiver>,
        zero_tx: Sender,
        ref_count: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            trigger_rx,
            shutdown_signal_trigger_rx,
            zero_tx,
            ref_count,
        }
    }

    /// Returns a Future that gets fulfilled when cancellation (shutdown) is requested
    /// and the delay (buffer) duration has been awaited on.
    ///
    /// Use [`Self::shutdown_signal_triggered`] in case you want to get
    /// a future which is triggered immediately when the shutdown signal is received,
    /// without waiting for the delay duration first.
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

    /// Returns a Future that gets fulfilled when cancellation (shutdown) is requested
    /// without awaiting the delay duration first, if one is set.
    ///
    /// In case no delay has been configured for the parent `Shutdown`,
    /// this function will be equal in behaviour to [`Self::cancelled`].
    ///
    /// Use [`Self::cancelled`] in case you want to get
    /// a future which is triggered when the shutdown signal is received
    /// and thethe delay duration is awaited.
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
    pub async fn shutdown_signal_triggered(&self) {
        self.shutdown_signal_trigger_rx
            .clone()
            .unwrap_or_else(|| self.trigger_rx.clone())
            .await
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
