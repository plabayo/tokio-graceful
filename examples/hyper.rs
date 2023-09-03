//! An example showcasing how to use [`tokio_graceful`] to gracefully shutdown a
//! [`tokio`] application which makes use of [`hyper`] (0.14).
//!
//! [`tokio_graceful`]: https://docs.rs/tokio-graceful
//! [`tokio`]: https://docs.rs/tokio
//! [`hyper`]: https://docs.rs/hyper/0.14/hyper

use std::convert::Infallible;
use std::time::Duration;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let shutdown = tokio_graceful::Shutdown::default();

    shutdown.spawn_task_fn(serve_tcp);

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
    // For every connection, we must make a `Service` to handle all
    // incoming HTTP requests on said connection.
    let make_svc = make_service_fn(|_conn| {
        // This is the `Service` that will handle the connection.
        // `service_fn` is a helper to convert a function that
        // returns a Response into a `Service`.
        async { Ok::<_, Infallible>(service_fn(hello)) }
    });

    let addr = ([127, 0, 0, 1], 8080).into();

    let server = Server::bind(&addr).serve(make_svc);
    let server = server.with_graceful_shutdown(shutdown_guard.clone_weak().into_cancelled());

    if let Err(err) = server.await {
        tracing::error!("server quit with error: {}", err);
    }
}

async fn hello(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    Ok(Response::new(Body::from("Hello World!")))
}
