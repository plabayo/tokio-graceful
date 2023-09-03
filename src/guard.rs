use std::{
    future::Future,
    sync::{atomic::AtomicUsize, Arc},
};

use tokio::task::JoinHandle;

use crate::trigger::{Receiver, Sender};

#[derive(Debug)]
pub struct ShutdownGuard(WeakShutdownGuard);

#[derive(Debug, Clone)]
pub struct WeakShutdownGuard {
    pub(crate) trigger_rx: Receiver,
    pub(crate) zero_tx: Sender,
    pub(crate) ref_count: Arc<AtomicUsize>,
}

impl ShutdownGuard {
    pub fn new(trigger_rx: Receiver, zero_tx: Sender, ref_count: Arc<AtomicUsize>) -> Self {
        let value = ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tracing::trace!("new shutdown guard: ref_count+1: {}", value + 1);
        Self(WeakShutdownGuard::new(trigger_rx, zero_tx, ref_count))
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
    pub fn new(trigger_rx: Receiver, zero_tx: Sender, ref_count: Arc<AtomicUsize>) -> Self {
        Self {
            trigger_rx,
            zero_tx,
            ref_count,
        }
    }

    #[inline]
    pub async fn cancelled(&self) {
        self.trigger_rx.clone().await;
    }

    #[inline]
    pub async fn into_cancelled(self) {
        self.cancelled().await;
    }

    #[inline]
    pub fn upgrade(self) -> ShutdownGuard {
        self.into()
    }
}
