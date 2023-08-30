use std::{
    future::{Future, IntoFuture},
    time,
};

use tokio_util::sync::CancellationToken;

pub fn pair() -> (Token, Handle) {
    pair_for(async {
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
    })
}

pub fn pair_for(signal: impl Future<Output = ()> + Send + 'static) -> (Token, Handle) {
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel(1);
    let cancellation_token = CancellationToken::new();
    {
        let cancellation_token = cancellation_token.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = signal => {
                    cancellation_token.cancel();
                }
                _ = cancellation_token.cancelled() => {}
            }
        });
    }
    (
        Token {
            cancellation_token: cancellation_token.clone(),
            shutdown_tx,
        },
        Handle {
            cancellation_token,
            shutdown_rx,
        },
    )
}

pub struct Token {
    cancellation_token: CancellationToken,
    shutdown_tx: tokio::sync::mpsc::Sender<()>,
}

impl Token {
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    pub fn child(&self) -> Self {
        Self {
            cancellation_token: self.cancellation_token.child_token(),
            shutdown_tx: self.shutdown_tx.clone(),
        }
    }

    pub async fn cancelled(&self) {
        self.cancellation_token.cancelled().await
    }
}

pub struct Handle {
    cancellation_token: CancellationToken,
    shutdown_rx: tokio::sync::mpsc::Receiver<()>,
}

impl Handle {
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    pub fn into_shutdown(self) -> ShutdownFuture {
        self.into_future()
    }

    pub fn into_graceful_shutdown(self, duration: time::Duration) -> ShutdownFuture {
        let mut fut = self.into_future();
        fut.delay = Some(tokio::time::sleep(duration));
        fut
    }
}

impl IntoFuture for Handle {
    type Output = ();
    type IntoFuture = ShutdownFuture;

    fn into_future(self) -> Self::IntoFuture {
        ShutdownFuture {
            shutdown_rx: self.shutdown_rx,
            delay: None,
        }
    }
}

pin_project_lite::pin_project! {
    pub struct ShutdownFuture {
        shutdown_rx: tokio::sync::mpsc::Receiver<()>,
        #[pin]
        delay: Option<tokio::time::Sleep>,
    }
}

impl Future for ShutdownFuture {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        let this = self.project();

        if this.shutdown_rx.poll_recv(cx).is_pending() {
            return std::task::Poll::Pending;
        }

        if let Some(delay) = this.delay.as_pin_mut() {
            if delay.poll(cx).is_pending() {
                return std::task::Poll::Pending;
            }
        }

        std::task::Poll::Ready(())
    }
}
