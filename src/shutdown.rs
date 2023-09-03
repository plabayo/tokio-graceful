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

    pub fn guard(&self) -> ShutdownGuard {
        self.guard.clone()
    }

    pub fn guard_weak(&self) -> WeakShutdownGuard {
        self.guard.clone_weak()
    }

    pub fn spawn_task<T>(&self, task: T) -> tokio::task::JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        self.guard.spawn_task(task)
    }

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
        self.guard.downgrade().cancelled().await;
        zero_notified.await;
    }

    pub async fn shutdown_with_limit(self, limit: time::Duration) -> Result<(), TimeoutError> {
        let zero_notified = self.notify_zero.notified();
        self.guard.downgrade().cancelled().await;
        tokio::select! {
            _ = tokio::time::sleep(limit) => { Err(TimeoutError) }
            _ = zero_notified => { Ok(()) }
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
pub struct TimeoutError;

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("timeout")
    }
}

impl std::error::Error for TimeoutError {}
