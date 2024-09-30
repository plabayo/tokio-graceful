use crate::{
    sync::JoinHandle,
    trigger::{trigger, Receiver},
    ShutdownGuard, WeakShutdownGuard,
};
use std::{
    fmt,
    future::Future,
    time::{self, Duration},
};

/// [`ShutdownBuilder`] to build a [`Shutdown`] manager.
pub struct ShutdownBuilder<T> {
    data: T,
}

impl Default for ShutdownBuilder<sealed::WithSignal<sealed::Default>> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for ShutdownBuilder<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShutdownBuilder")
            .field("data", &self.data)
            .finish()
    }
}

impl ShutdownBuilder<sealed::WithSignal<sealed::Default>> {
    /// Create a new [`ShutdownBuilder`], which by default
    /// is ready to build a [`Shutdown`].
    pub fn new() -> Self {
        Self {
            data: sealed::WithSignal {
                signal: sealed::Default,
                delay: None,
            },
        }
    }

    /// Create a [`ShutdownBuilder`] without a trigger signal,
    /// meaning it will act like a WaitGroup.
    pub fn without_signal(self) -> ShutdownBuilder<sealed::WithoutSignal> {
        ShutdownBuilder {
            data: sealed::WithoutSignal,
        }
    }

    /// Create a [`ShutdownBuilder`] with a custom [`Future`] signal.
    pub fn with_signal<F: Future + Send + 'static>(
        self,
        future: F,
    ) -> ShutdownBuilder<sealed::WithSignal<F>> {
        ShutdownBuilder {
            data: sealed::WithSignal {
                signal: future,
                delay: self.data.delay,
            },
        }
    }
}

impl<S> ShutdownBuilder<sealed::WithSignal<S>> {
    /// Create a [`ShutdownBuilder`] with a function
    /// which creates a future that will be awaited on
    /// as an alternative to waiting for all jobs to be complete.
    pub fn with_overwrite_fn<F, Fut>(
        self,
        f: F,
    ) -> ShutdownBuilder<sealed::WithSignalAndOverwriteFn<S, F>>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future + Send + 'static,
    {
        ShutdownBuilder {
            data: sealed::WithSignalAndOverwriteFn {
                signal: self.data.signal,
                overwrite_fn: f,
                delay: self.data.delay,
            },
        }
    }

    /// Attach a delay to this [`ShutdownBuilder`]
    /// which will used as a timeout buffer between the shutdown
    /// trigger signal and signalling the jobs to be cancelled.
    pub fn with_delay(self, delay: Duration) -> Self {
        Self {
            data: sealed::WithSignal {
                signal: self.data.signal,
                delay: Some(delay),
            },
        }
    }

    /// Attach a delay to this [`ShutdownBuilder`]
    /// which will used as a timeout buffer between the shutdown
    /// trigger signal and signalling the jobs to be cancelled.
    pub fn maybe_with_delay(self, delay: Option<Duration>) -> Self {
        Self {
            data: sealed::WithSignal {
                signal: self.data.signal,
                delay,
            },
        }
    }

    /// Attach a delay to this [`ShutdownBuilder`]
    /// which will used as a timeout buffer between the shutdown
    /// trigger signal and signalling the jobs to be cancelled.
    pub fn set_delay(&mut self, delay: Duration) -> &mut Self {
        self.data.delay = Some(delay);
        self
    }
}

impl<S, F> ShutdownBuilder<sealed::WithSignalAndOverwriteFn<S, F>> {
    /// Attach a delay to this [`ShutdownBuilder`]
    /// which will used as a timeout buffer between the shutdown
    /// trigger signal and signalling the jobs to be cancelled.
    pub fn with_delay(self, delay: Duration) -> Self {
        Self {
            data: sealed::WithSignalAndOverwriteFn {
                signal: self.data.signal,
                overwrite_fn: self.data.overwrite_fn,
                delay: Some(delay),
            },
        }
    }

    /// Attach a delay to this [`ShutdownBuilder`]
    /// which will used as a timeout buffer between the shutdown
    /// trigger signal and signalling the jobs to be cancelled.
    pub fn maybe_with_delay(self, delay: Option<Duration>) -> Self {
        Self {
            data: sealed::WithSignalAndOverwriteFn {
                signal: self.data.signal,
                overwrite_fn: self.data.overwrite_fn,
                delay,
            },
        }
    }

    /// Attach a delay to this [`ShutdownBuilder`]
    /// which will used as a timeout buffer between the shutdown
    /// trigger signal and signalling the jobs to be cancelled.
    pub fn set_delay(&mut self, delay: Duration) -> &mut Self {
        self.data.delay = Some(delay);
        self
    }
}

impl ShutdownBuilder<sealed::WithoutSignal> {
    /// Build a [`Shutdown`] that acts like a WaitGroup.
    pub fn build(self) -> Shutdown {
        let (zero_tx, zero_rx) = trigger();

        let guard = ShutdownGuard::new(Receiver::closed(), None, zero_tx, Default::default());

        Shutdown {
            guard,
            zero_rx,
            zero_overwrite_rx: Receiver::pending(),
        }
    }
}

impl<I: sealed::IntoFuture> ShutdownBuilder<sealed::WithSignal<I>> {
    /// Build a [`Shutdown`] which will allow a shutdown
    /// when the shutdown signal has been triggered AND
    /// all jobs are complete.
    pub fn build(self) -> Shutdown {
        let trigger_signal = self.data.signal.into_future();

        let (delay_tuple, maybe_shutdown_signal_rx) = match self.data.delay {
            Some(delay) => {
                let (shutdown_signal_tx, shutdown_signal_rx) = trigger();
                (Some((delay, shutdown_signal_tx)), Some(shutdown_signal_rx))
            }
            None => (None, None),
        };

        let (signal_tx, signal_rx) = trigger();
        let (zero_tx, zero_rx) = trigger();

        let guard = ShutdownGuard::new(
            signal_rx,
            maybe_shutdown_signal_rx,
            zero_tx,
            Default::default(),
        );

        crate::sync::spawn(async move {
            let _ = trigger_signal.await;
            if let Some((delay, shutdown_signal_tx)) = delay_tuple {
                shutdown_signal_tx.trigger();
                tracing::trace!(
                    "::trigger signal recieved: delay buffer activated: {:?}",
                    delay
                );
                tokio::time::sleep(delay).await;
            }
            signal_tx.trigger();
        });

        Shutdown {
            guard,
            zero_rx,
            zero_overwrite_rx: Receiver::pending(),
        }
    }
}

impl<I, F, Fut> ShutdownBuilder<sealed::WithSignalAndOverwriteFn<I, F>>
where
    I: sealed::IntoFuture,
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future + Send + 'static,
{
    /// Build a [`Shutdown`] which will allow a shutdown
    /// when the shutdown signal has been triggered AND
    /// either all jobs are complete or the overwrite (force)
    /// signal has been triggered instead.
    pub fn build(self) -> Shutdown {
        let trigger_signal = self.data.signal.into_future();
        let overwrite_fn = self.data.overwrite_fn;

        let (delay_tuple, maybe_shutdown_signal_rx) = match self.data.delay {
            Some(delay) => {
                let (shutdown_signal_tx, shutdown_signal_rx) = trigger();
                (Some((delay, shutdown_signal_tx)), Some(shutdown_signal_rx))
            }
            None => (None, None),
        };

        let (signal_tx, signal_rx) = trigger();
        let (zero_tx, zero_rx) = trigger();
        let (zero_overwrite_tx, zero_overwrite_rx) = trigger();

        let guard = ShutdownGuard::new(
            signal_rx,
            maybe_shutdown_signal_rx,
            zero_tx,
            Default::default(),
        );

        crate::sync::spawn(async move {
            let _ = trigger_signal.await;
            let overwrite_signal = overwrite_fn();
            crate::sync::spawn(async move {
                let _ = overwrite_signal.await;
                zero_overwrite_tx.trigger();
            });
            if let Some((delay, shutdown_signal_tx)) = delay_tuple {
                shutdown_signal_tx.trigger();
                tracing::trace!(
                    "::trigger signal recieved: delay buffer activated: {:?}",
                    delay
                );
                tokio::time::sleep(delay).await;
            }
            signal_tx.trigger();
        });

        Shutdown {
            guard,
            zero_rx,
            zero_overwrite_rx,
        }
    }
}

/// The [`Shutdown`] struct is the main entry point to the shutdown system.
///
/// It is created by calling [`Shutdown::new`], which takes a [`Future`] that
/// will be awaited on when shutdown is requested. Most users will want to
/// create a [`Shutdown`] with [`Shutdown::default`], which uses the default
/// signal handler to trigger shutdown. See [`default_signal`] for more info.
///
/// > (NOTE: that these defaults are not available when compiling with --cfg loom)
///
/// See the [README] for more info on how to use this crate.
///
/// [`Future`]: std::future::Future
/// [README]: https://github.com/plabayo/tokio-graceful/blob/main/README.md
pub struct Shutdown {
    guard: ShutdownGuard,
    zero_rx: Receiver,
    zero_overwrite_rx: Receiver,
}

impl Shutdown {
    /// Create a [`ShutdownBuilder`] allowing you to add a delay,
    /// a custom shutdown trigger signal and even an overwrite signal
    /// to force a shutdown even if workers are still busy.
    pub fn builder() -> ShutdownBuilder<sealed::WithSignal<sealed::Default>> {
        ShutdownBuilder::default()
    }

    /// Creates a new [`Shutdown`] struct with the given [`Future`].
    ///
    /// The [`Future`] will be awaited on when shutdown is requested.
    ///
    /// [`Future`]: std::future::Future
    pub fn new(signal: impl Future + Send + 'static) -> Self {
        ShutdownBuilder::default().with_signal(signal).build()
    }

    /// Creates a new [`Shutdown`] struct with no signal.
    ///
    /// This is useful if you want to support a Waitgroup
    /// like system where you wish to wait for all open tasks
    /// without requiring a signal to be triggered first.
    pub fn no_signal() -> Self {
        ShutdownBuilder::default().without_signal().build()
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

    /// Returns a Tokio [`crate::sync::JoinHandle`] that can be awaited on
    /// to wait for the spawned task to complete. See
    /// [`crate::sync::spawn`] for more information.
    #[inline]
    pub fn spawn_task<T>(&self, task: T) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        self.guard.spawn_task(task)
    }

    /// Returns a Tokio [`crate::sync::JoinHandle`] that can be awaited on
    /// to wait for the spawned task (fn) to complete. See
    /// [`crate::sync::spawn`] for more information.
    #[inline]
    pub fn spawn_task_fn<T, F>(&self, task: F) -> JoinHandle<T::Output>
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
    pub async fn shutdown(mut self) -> time::Duration {
        tracing::info!("::shutdown: waiting for signal to trigger (read: to be cancelled)");
        let weak_guard = self.guard.downgrade();
        let start: time::Instant = time::Instant::now();
        tokio::select! {
            _ = weak_guard.cancelled() => {
                tracing::info!("::shutdown: waiting for all guards to drop");
            }
            _ = &mut self.zero_overwrite_rx => {
                let elapsed = start.elapsed();
                tracing::warn!("::shutdown: enforced: overwrite delayed cancellation after {}s", elapsed.as_secs_f64());
                return elapsed;
            }
        };

        let start: time::Instant = time::Instant::now();
        tokio::select! {
            _ = self.zero_rx => {
                let elapsed = start.elapsed();
                tracing::info!("::shutdown: ready after {}s", elapsed.as_secs_f64());
                elapsed
            }
            _ = self.zero_overwrite_rx => {
                let elapsed = start.elapsed();
                tracing::warn!("::shutdown: enforced: overwrite signal triggered after {}s", elapsed.as_secs_f64());
                elapsed
            }
        }
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
        mut self,
        limit: time::Duration,
    ) -> Result<time::Duration, TimeoutError> {
        tracing::info!("::shutdown: waiting for signal to trigger (read: to be cancelled)");
        let weak_guard = self.guard.downgrade();
        let start: time::Instant = time::Instant::now();
        tokio::select! {
            _ = weak_guard.cancelled() => {
                tracing::info!(
                    "::shutdown: waiting for all guards to drop for a max of {}s",
                    limit.as_secs_f64()
                );
            }
            _ = &mut self.zero_overwrite_rx => {
                let elapsed = start.elapsed();
                tracing::warn!("::shutdown: enforced: overwrite delayed cancellation after {}s", elapsed.as_secs_f64());
                return Err(TimeoutError(elapsed));
            }
        };

        let start: time::Instant = time::Instant::now();
        tokio::select! {
            _ = tokio::time::sleep(limit) => {
                tracing::info!("::shutdown: timeout after {}s", limit.as_secs_f64());
                Err(TimeoutError(limit))
            }
            _ = self.zero_rx => {
                let elapsed = start.elapsed();
                tracing::info!("::shutdown: ready after {}s", elapsed.as_secs_f64());
                Ok(elapsed)
            }
            _ = self.zero_overwrite_rx => {
                let elapsed = start.elapsed();
                tracing::warn!("::shutdown: enforced: overwrite signal triggered after {}s", elapsed.as_secs_f64());
                Err(TimeoutError(elapsed))
            }
        }
    }
}

/// Returns a [`Future`] that completes once one of the default signals.
///
/// Which on Unix is Ctrl-C (sigint) or sigterm,
/// and on Windows is Ctrl-C, Ctrl-Close or Ctrl-Shutdown.
///
/// Exposed to you so you can easily expand it by for example
/// chaining it with a [`tokio::time::sleep`] to have a delay
/// before shutdown is triggered.
///
/// [`Future`]: std::future::Future
/// [`tokio::time::sleep`]: https://docs.rs/tokio/*/tokio/time/fn.sleep.html
#[cfg(all(not(loom), any(unix, windows)))]
pub async fn default_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(all(unix, not(windows)))]
    {
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
    #[cfg(all(not(unix), windows))]
    {
        let ctrl_close = async {
            let mut signal = tokio::signal::windows::ctrl_close()?;
            signal.recv().await;
            std::io::Result::Ok(())
        };
        let ctrl_shutdown = async {
            let mut signal = tokio::signal::windows::ctrl_shutdown()?;
            signal.recv().await;
            std::io::Result::Ok(())
        };
        tokio::select! {
            _ = ctrl_c => {}
            _ = ctrl_close => {}
            _ = ctrl_shutdown => {}
        }
    }
}

#[cfg(all(not(loom), any(unix, windows)))]
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

mod sealed {
    use std::{fmt, future::Future, time::Duration};

    pub trait IntoFuture: Send + 'static {
        fn into_future(self) -> impl Future + Send + 'static;
    }

    impl<F> IntoFuture for F
    where
        F: Future + Send + 'static,
    {
        fn into_future(self) -> impl Future + Send + 'static {
            self
        }
    }

    #[derive(Debug)]
    #[non_exhaustive]
    pub struct Default;

    impl IntoFuture for Default {
        #[cfg(loom)]
        fn into_future(self) -> impl Future + Send + 'static {
            std::future::pending::<()>()
        }
        #[cfg(not(loom))]
        fn into_future(self) -> impl Future + Send + 'static {
            super::default_signal()
        }
    }

    #[derive(Debug)]
    #[non_exhaustive]
    pub struct WithoutSignal;

    pub struct WithSignal<S> {
        pub(super) signal: S,
        pub(super) delay: Option<Duration>,
    }

    impl<S: fmt::Debug> fmt::Debug for WithSignal<S> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("WithSignal")
                .field("signal", &self.signal)
                .field("delay", &self.delay)
                .finish()
        }
    }

    pub struct WithSignalAndOverwriteFn<S, F> {
        pub(super) signal: S,
        pub(super) overwrite_fn: F,
        pub(super) delay: Option<Duration>,
    }

    impl<S: fmt::Debug, F: fmt::Debug> fmt::Debug for WithSignalAndOverwriteFn<S, F> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("WithSignalAndOverwriteFn")
                .field("signal", &self.signal)
                .field("overwrite_fn", &self.overwrite_fn)
                .field("delay", &self.delay)
                .finish()
        }
    }
}
