use std::{
    future::Future,
    sync::{atomic::AtomicUsize, Arc},
};

use tokio::{sync::Notify, task::JoinHandle};

#[derive(Debug)]
pub struct ShutdownGuard(WeakShutdownGuard);

#[derive(Debug, Clone)]
pub struct WeakShutdownGuard {
    pub(crate) notify_signal: Arc<Notify>,
    pub(crate) notify_zero: Arc<Notify>,
    pub(crate) ref_count: Arc<AtomicUsize>,
}

impl ShutdownGuard {
    pub fn new(
        notify_signal: Arc<Notify>,
        notify_zero: Arc<Notify>,
        ref_count: Arc<AtomicUsize>,
    ) -> Self {
        let value = ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tracing::trace!("new shutdown guard: ref_count: {}", value + 1);
        Self(WeakShutdownGuard::new(
            notify_signal,
            notify_zero,
            ref_count,
        ))
    }

    #[inline]
    pub async fn cancelled(&self) {
        self.0.cancelled().await
    }

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

    pub fn into_spawn_task<T>(self, task: T) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        self.spawn_task(task)
    }

    pub fn spawn_task_fn<F, T>(&self, task: F) -> JoinHandle<T::Output>
    where
        F: FnOnce(ShutdownGuard) -> T + Send + 'static,
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let guard = self.clone();
        tokio::spawn(async move { task(guard).await })
    }

    pub fn into_spawn_task_fn<F, T>(self, task: F) -> JoinHandle<T::Output>
    where
        F: FnOnce(ShutdownGuard) -> T + Send + 'static,
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        self.spawn_task_fn(task)
    }

    pub fn downgrade(self) -> WeakShutdownGuard {
        self.clone_weak()
    }

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
        tracing::trace!("clone shutdown guard: ref_count: {}", value + 1);
        Self(self.0.clone())
    }
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        let cnt = self
            .0
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        tracing::trace!("drop shutdown guard: ref_count: {}", cnt - 1);
        if cnt == 1 {
            self.0.notify_zero.notify_one();
        }
    }
}

impl WeakShutdownGuard {
    pub fn new(
        notify_signal: Arc<Notify>,
        notify_zero: Arc<Notify>,
        ref_count: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            notify_signal,
            notify_zero,
            ref_count,
        }
    }

    #[inline]
    pub async fn cancelled(&self) {
        self.notify_signal.notified().await;
    }

    pub fn upgrade(self) -> Option<ShutdownGuard> {
        if self.ref_count.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            None
        } else {
            self.ref_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Some(ShutdownGuard(self))
        }
    }
}
