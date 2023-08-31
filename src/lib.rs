use std::{future::Future, time};

use tokio_util::sync::CancellationToken;

pub fn token() -> TokenHandle {
    TokenHandle::default()
}

pub fn token_for(signal: impl Future<Output = ()> + Send + 'static) -> TokenHandle {
    TokenHandle::new(signal)
}

#[derive(Debug)]
pub struct TokenHandle {
    cancellation_token: CancellationToken,
    shutdown_tx: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    shutdown_rx: tokio::sync::mpsc::UnboundedReceiver<()>,
}

impl TokenHandle {
    pub fn new(signal: impl Future<Output = ()> + Send + 'static) -> Self {
        let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
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
        let shutdown_tx = Some(shutdown_tx);
        Self {
            cancellation_token,
            shutdown_tx,
            shutdown_rx,
        }
    }

    pub fn token(&self) -> Token {
        tracing::trace!("shutdown: create new token");
        Token {
            cancellation_token: self.cancellation_token.child_token(),
            _shutdown_tx: self.shutdown_tx.clone(),
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), TimeoutError> {
        tracing::trace!("shutdown: wait for cancellation");
        self.cancellation_token.cancelled().await;
        std::mem::take(&mut self.shutdown_tx);

        let shutdown_fut = self.shutdown_rx.recv();
        tokio::pin!(shutdown_fut);

        tracing::trace!("shutdown_with_delay: wait for tokens or immediately quit");
        match futures_util::future::select(shutdown_fut, std::future::ready(())).await {
            futures_util::future::Either::Left((_, _)) => Ok(()),
            futures_util::future::Either::Right((_, _)) => Err(TimeoutError),
        }
    }

    pub async fn shutdown_with_delay(&mut self, delay: time::Duration) -> Result<(), TimeoutError> {
        tracing::trace!("shutdown_with_delay: wait for cancellation");
        self.cancellation_token.cancelled().await;
        std::mem::take(&mut self.shutdown_tx);

        let shutdown_fut = self.shutdown_rx.recv();
        tokio::pin!(shutdown_fut);

        let sleep = tokio::time::sleep(delay);
        tokio::pin!(sleep);

        tracing::trace!("shutdown_with_delay: wait for tokens or sleep");
        match futures_util::future::select(shutdown_fut, sleep).await {
            futures_util::future::Either::Left((_, _)) => Ok(()),
            futures_util::future::Either::Right((_, _)) => Err(TimeoutError),
        }
    }
}

impl Default for TokenHandle {
    fn default() -> Self {
        Self::new(async {
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
}

#[derive(Debug, Clone)]
pub struct Token {
    cancellation_token: CancellationToken,
    _shutdown_tx: Option<tokio::sync::mpsc::UnboundedSender<()>>,
}

impl Token {
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    pub async fn cancelled(&self) {
        self.cancellation_token.cancelled().await
    }

    pub async fn cancelled_with_limit(&self, limit: time::Duration) {
        tokio::select! {
            _ = self.cancelled() => {}
            _ = tokio::time::sleep(limit) => {}
        }
    }

    pub async fn cancelled_with_delay(&self, delay: time::Duration) {
        self.cancelled().await;
        tokio::time::sleep(delay).await
    }

    pub fn child(&self) -> Self {
        Self {
            cancellation_token: self.cancellation_token.child_token(),
            _shutdown_tx: self._shutdown_tx.clone(),
        }
    }
}

#[derive(Debug)]
pub struct TimeoutError;

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "timeout".fmt(f)
    }
}

impl std::error::Error for TimeoutError {}
