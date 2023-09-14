use std::{future::Future, time};

use crate::{
    sync::Arc,
    trigger::{trigger, Receiver},
    ShutdownGuard, WeakShutdownGuard,
};

/// The [`Shutdown`] struct is the main entry point to the shutdown system.
///
/// It is created by calling [`Shutdown::new`], which takes a [`Future`] that
/// will be awaited on when shutdown is requested. Most users will want to
/// create a [`Shutdown`] with [`Shutdown::default`], which uses the default
/// signal handler to trigger shutdown. See [`default_signal`] for more info.
///
/// See the [README] for more info on how to use this crate.
///
/// [`Future`]: std::future::Future
/// [README]: https://github.com/plabayo/tokio-graceful/blob/main/README.md
pub struct Shutdown {
    guard: ShutdownGuard,
    zero_rx: Receiver,
}

impl Shutdown {
    /// Creates a new [`Shutdown`] struct with the given [`Future`].
    ///
    /// The [`Future`] will be awaited on when shutdown is requested.
    ///
    /// [`Future`]: std::future::Future
    pub fn new(signal: impl Future<Output = ()> + Send + 'static) -> Self {
        let (signal_tx, signal_rx) = trigger();
        let (zero_tx, zero_rx) = trigger();

        let guard = ShutdownGuard::new(signal_rx, zero_tx, Arc::new(0usize.into()));

        tokio::spawn(async move {
            signal.await;
            signal_tx.trigger();
        });

        Self { guard, zero_rx }
    }

    /// Creates a new [`Shutdown`] struct with no signal.
    ///
    /// This is useful if you want to support a Waitgroup
    /// like system where you wish to wait for all open tasks
    /// without requiring a signal to be triggered first.
    pub fn no_signal() -> Self {
        Self::new(async {})
    }

    /// Returns a [`ShutdownGuard`] which primary use
    /// is to prevent the [`Shutdown`] from shutting down.
    ///
    /// The creation of a [`ShutdownGuard`] is lockfree.
    ///
    /// [`ShutdownGuard`]: crate::ShutdownGuard
    #[inline]
    pub fn guard(&self) -> ShutdownGuard {
        self.guard.clone()
    }

    /// Returns a [`WeakShutdownGuard`] which in contrast to
    /// [`ShutdownGuard`] does not prevent the [`Shutdown`]
    /// from shutting down.
    ///
    /// Instead it is used to wait for
    /// "shutdown signal" to be triggered or to create
    /// a [`ShutdownGuard`] which prevents the [`Shutdown`]
    /// once and only once it is needed.
    ///
    /// The creation of a [`WeakShutdownGuard`] is lockfree.
    ///
    /// [`ShutdownGuard`]: crate::ShutdownGuard
    /// [`WeakShutdownGuard`]: crate::WeakShutdownGuard
    /// [`Shutdown`]: crate::Shutdown
    #[inline]
    pub fn guard_weak(&self) -> WeakShutdownGuard {
        self.guard.clone_weak()
    }

    /// Returns a Tokio [`JoinHandle`] that can be awaited on
    /// to wait for the spawned task to complete. See
    /// [`tokio::spawn`] for more information.
    ///
    /// [`JoinHandle`]: https://docs.rs/tokio/*/tokio/task/struct.JoinHandle.html
    /// [`tokio::spawn`]: https://docs.rs/tokio/*/tokio/task/fn.spawn.html
    #[inline]
    pub fn spawn_task<T>(&self, task: T) -> tokio::task::JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        self.guard.spawn_task(task)
    }

    /// Returns a Tokio [`JoinHandle`] that can be awaited on
    /// to wait for the spawned task (fn) to complete. See
    /// [`tokio::spawn`] for more information.
    ///
    /// [`JoinHandle`]: https://docs.rs/tokio/*/tokio/task/struct.JoinHandle.html
    /// [`tokio::spawn`]: https://docs.rs/tokio/*/tokio/task/fn.spawn.html
    #[inline]
    pub fn spawn_task_fn<T, F>(&self, task: F) -> tokio::task::JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
        F: FnOnce(ShutdownGuard) -> T + Send + 'static,
    {
        self.guard.spawn_task_fn(task)
    }

    /// Returns a future that completes once the [`Shutdown`] has been triggered
    /// and all [`ShutdownGuard`]s have been dropped.
    ///
    /// The resolved [`Duration`] is the time it took for the [`Shutdown`] to
    /// to wait for all [`ShutdownGuard`]s to be dropped.
    ///
    /// You can use [`Shutdown::shutdown_with_limit`] to limit the time the
    /// [`Shutdown`] waits for all [`ShutdownGuard`]s to be dropped.
    ///
    /// # Panics
    ///
    /// This method can panic if the internal mutex is poisoned.
    ///
    /// [`ShutdownGuard`]: crate::ShutdownGuard
    /// [`Duration`]: std::time::Duration
    pub async fn shutdown(self) -> time::Duration {
        tracing::trace!("::shutdown: waiting for signal to trigger (read: to be cancelled)");
        self.guard.downgrade().cancelled().await;
        tracing::trace!("::shutdown: waiting for all guards to drop");
        let start: time::Instant = time::Instant::now();
        self.zero_rx.await;
        let elapsed = start.elapsed();
        tracing::trace!("::shutdown: ready after {}s", elapsed.as_secs_f64());
        elapsed
    }

    /// Returns a future that completes once the [`Shutdown`] has been triggered
    /// and all [`ShutdownGuard`]s have been dropped or the given [`Duration`]
    /// has elapsed.
    ///
    /// The resolved [`Duration`] is the time it took for the [`Shutdown`] to
    /// to wait for all [`ShutdownGuard`]s to be dropped.
    ///
    /// You can use [`Shutdown::shutdown`] to wait for all [`ShutdownGuard`]s
    /// to be dropped without a time limit.
    ///
    /// # Panics
    ///
    /// This method can panic if the internal mutex is poisoned.
    ///
    /// [`ShutdownGuard`]: crate::ShutdownGuard
    /// [`Duration`]: std::time::Duration
    pub async fn shutdown_with_limit(
        self,
        limit: time::Duration,
    ) -> Result<time::Duration, TimeoutError> {
        tracing::trace!("::shutdown: waiting for signal to trigger (read: to be cancelled)");
        self.guard.downgrade().cancelled().await;
        tracing::trace!(
            "::shutdown: waiting for all guards to drop for a max of {}s",
            limit.as_secs_f64()
        );
        let start: time::Instant = time::Instant::now();
        tokio::select! {
            _ = tokio::time::sleep(limit) => {
                tracing::trace!("::shutdown: timeout after {}s", limit.as_secs_f64());
                Err(TimeoutError(limit))
            }
            _ = self.zero_rx => {
                let elapsed = start.elapsed();
                tracing::trace!("::shutdown: ready after {}s", elapsed.as_secs_f64());
                Ok(elapsed)
            }
        }
    }
}

/// Returns a [`Future`] that completes once one of the default signals
/// (SIGINT and CTRL-C) are received.
///
/// Exposed to you so you can easily expand it by for example
/// chaining it with a [`tokio::time::sleep`] to have a delay
/// before shutdown is triggered.
///
/// [`Future`]: std::future::Future
/// [`tokio::time::sleep`]: https://docs.rs/tokio/*/tokio/time/fn.sleep.html
pub async fn default_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    let signal = async {
        let mut os_signal =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        os_signal.recv().await;
        std::io::Result::Ok(())
    };
    tokio::select! {
        _ = ctrl_c => {}
        _ = signal => {}
    }
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new(default_signal())
    }
}

#[derive(Debug)]
pub struct TimeoutError(time::Duration);

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "timeout after {}s", self.0.as_secs_f64())
    }
}

impl std::error::Error for TimeoutError {}
