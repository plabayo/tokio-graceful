use std::{future::Future, sync::Arc, time};

use tokio::sync::Notify;

use crate::{ShutdownGuard, WeakShutdownGuard};

pub struct Shutdown {
    guard: ShutdownGuard,
    notify_zero: Arc<Notify>,
}

impl Shutdown {
    pub fn new(signal: impl Future<Output = ()> + Send + 'static) -> Self {
        let notify_signal = Arc::new(Notify::new());
        let notify_zero = Arc::new(Notify::new());
        let guard = ShutdownGuard::new(
            notify_signal.clone(),
            notify_zero.clone(),
            Arc::new(0usize.into()),
        );

        tokio::spawn(async move {
            signal.await;
            notify_signal.notify_waiters();
        });

        Self { guard, notify_zero }
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

    pub async fn shutdown(self) {
        let zero_notified = self.notify_zero.notified();
        tracing::trace!("::shutdown: waiting for signal to trigger (read: to be cancelled)");
        self.guard.downgrade().cancelled().await;
        tracing::trace!("::shutdown: waiting for all guards to drop");
        zero_notified.await;
        tracing::trace!("::shutdown: ready");
    }

    pub async fn shutdown_with_limit(
        self,
        limit: time::Duration,
    ) -> Result<time::Duration, TimeoutError> {
        let zero_notified = self.notify_zero.notified();
        tracing::trace!("::shutdown: waiting for signal to trigger (read: to be cancelled)");
        self.guard.downgrade().cancelled().await;
        tracing::trace!(
            "::shutdown: waiting for all guards to drop for a max of {}s",
            limit.as_secs_f64()
        );
        let start: time::Instant = time::Instant::now();
        tokio::select! {
            _ = tokio::time::sleep(limit) => { Err(TimeoutError(limit)) }
            _ = zero_notified => { Ok(start.elapsed()) }
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
