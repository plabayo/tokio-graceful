use std::{future::Future, sync::Arc, time};

use crate::{
    trigger::{trigger, Receiver},
    ShutdownGuard, WeakShutdownGuard,
};

pub struct Shutdown {
    guard: ShutdownGuard,
    zero_rx: Receiver,
}

impl Shutdown {
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

    #[inline]
    pub fn guard(&self) -> ShutdownGuard {
        self.guard.clone()
    }

    #[inline]
    pub fn guard_weak(&self) -> WeakShutdownGuard {
        self.guard.clone_weak()
    }

    #[inline]
    pub fn spawn_task<T>(&self, task: T) -> tokio::task::JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        self.guard.spawn_task(task)
    }

    #[inline]
    pub fn spawn_task_fn<T, F>(&self, task: F) -> tokio::task::JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
        F: FnOnce(ShutdownGuard) -> T + Send + 'static,
    {
        self.guard.spawn_task_fn(task)
    }

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
