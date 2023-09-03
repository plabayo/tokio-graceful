//! An example showcasing how to use [`tokio_graceful`] to gracefully shutdown a
//! [`tokio`] application which makes use of [`tokio::net::TcpListener`].
//!
//! [`tokio_graceful`]: https://docs.rs/tokio-graceful
//! [`tokio`]: https://docs.rs/tokio
//! [`tokio::net::TcpListener`]: https://docs.rs/tokio/latest/tokio/net/struct.TcpListener.html

use std::time::Duration;

use tokio::net::TcpListener;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let shutdown = tokio_graceful::Shutdown::default();

    shutdown.spawn_task_fn(serve_tcp);

    if shutdown
        .shutdown_with_limit(Duration::from_secs(10))
        .await
        .is_err()
    {
        tracing::warn!("shutdown: forcefully due to timeout");
    }

    tracing::info!("Bye!");
}

async fn serve_tcp(shutdown_guard: tokio_graceful::ShutdownGuard) {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    loop {
        let shutdown_guard = shutdown_guard.clone();
        tokio::select! {
            _ = shutdown_guard.cancelled() => {
                tracing::info!("signal received: initiate graceful shutdown");
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((socket, _)) => {
                        tokio::spawn(async move {
                            let (mut reader, mut writer) = tokio::io::split(socket);
                            let _ = tokio::io::copy(&mut reader, &mut writer).await;
                            drop(shutdown_guard);
                        });
                    }
                    Err(e) => {
                        tracing::warn!("accept error: {:?}", e);
                    }
                }
            }
        }
    }
}
