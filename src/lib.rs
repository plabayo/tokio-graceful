#![doc = include_str!("../README.md")]

mod guard;
pub use guard::{ShutdownGuard, WeakShutdownGuard};

mod shutdown;
pub use shutdown::Shutdown;

pub(crate) mod trigger;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::oneshot;

    use super::*;

    #[tokio::test]
    async fn test_shutdown_nope() {
        let (tx, rx) = oneshot::channel::<()>();
        let shutdown = Shutdown::new(async {
            rx.await.unwrap();
        });
        tokio::spawn(async move {
            tx.send(()).unwrap();
        });
        shutdown.shutdown().await;
    }

    #[tokio::test]
    async fn test_shutdown_nope_limit() {
        let (tx, rx) = oneshot::channel::<()>();
        let shutdown = Shutdown::new(async {
            rx.await.unwrap();
        });
        tokio::spawn(async move {
            tx.send(()).unwrap();
        });
        shutdown
            .shutdown_with_limit(Duration::from_secs(60))
            .await
            .unwrap();
    }
}
