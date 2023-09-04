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

    // Short for `shutdown.guard().into_spawn_task_fn(serve_tcp)`
    // In case you only wish to pass in a future (in contrast to a function)
    // as you do not care about being able to use the linked guard,
    // you can also use [`Shutdown::spawn_task`](https://docs.rs/tokio-graceful/latest/tokio_graceful/struct.Shutdown.html#method.spawn_task).
    shutdown.spawn_task_fn(serve_tcp);

    // use [`Shutdown::shutdown`](https://docs.rs/tokio-graceful/latest/tokio_graceful/struct.Shutdown.html#method.shutdown)
    // to wait for all guards to drop without any limit on how long to wait.
    match shutdown.shutdown_with_limit(Duration::from_secs(10)).await {
        Ok(elapsed) => {
            tracing::info!(
                "shutdown: gracefully {}s after shutdown signal received",
                elapsed.as_secs_f64()
            );
        }
        Err(e) => {
            tracing::warn!("shutdown: forcefully due to timeout: {}", e);
        }
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
                            // NOTE, make sure to pass a clone of the shutdown guard to this function
                            // or any of its children in case you wish to be able to cancel a long running process should the
                            // shutdown signal be received and you know that your task might not finish on time.
                            // This allows you to at least leave it behind in a consistent state such that another
                            // process can pick up where you left that task.
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
