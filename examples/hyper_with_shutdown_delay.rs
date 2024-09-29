//! An example showcasing how to use [`tokio_graceful`] to gracefully shutdown a
//! [`tokio`] application which makes use of [`hyper`] (0.14).
//!
//! Libraries such as [`axum`] are built on top of Hyper and thus
//! [`tokio_graceful`] can be used to gracefully shutdown applications built on
//! top of them.
//!
//! [`tokio_graceful`]: https://docs.rs/tokio-graceful
//! [`tokio`]: https://docs.rs/tokio
//! [`hyper`]: https://docs.rs/hyper/0.14/hyper
//! [`axum`]: https://docs.rs/axum

use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1::Builder;
use hyper::service::service_fn;
use hyper::{body::Incoming, Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let shutdown = tokio_graceful::Shutdown::builder()
        .with_delay(Duration::from_secs(5))
        .build();

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
    let addr: SocketAddr = ([127, 0, 0, 1], 8080).into();

    let listener = TcpListener::bind(&addr).await.unwrap();

    loop {
        let stream = tokio::select! {
            _ = shutdown_guard.cancelled() => {
                tracing::info!("signal received: initiate graceful shutdown");
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        stream
                    }
                    Err(e) => {
                        tracing::warn!("accept error: {:?}", e);
                        continue;
                    }
                }
            }
        };
        let stream = TokioIo::new(stream);

        shutdown_guard.spawn_task_fn(move |guard: tokio_graceful::ShutdownGuard| async move {
            let conn = Builder::new()
                .serve_connection(stream, service_fn(hello));
            let mut conn = std::pin::pin!(conn);

            tokio::select! {
                _ = guard.cancelled() => {
                    tracing::info!("signal received: initiate graceful shutdown");
                    conn.as_mut().graceful_shutdown();
                }
                result = conn.as_mut() => {
                    if let Err(err) = result {
                        tracing::error!(error = &err as &dyn std::error::Error, "conn exited with error");
                    }
                    return;
                }
            }
            if let Err(err) = conn.as_mut().await  {
                tracing::error!(error = &err as &dyn std::error::Error, "conn exited with error after graceful shutdown");
            }
        });
    }
}

async fn hello(_: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    Ok(Response::new(Full::from("Hello World!")))
}
