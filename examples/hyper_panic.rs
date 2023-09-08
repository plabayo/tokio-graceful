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

    // oneshot channel used to be able to trigger a shutdown
    // in case of an unexpected task exit
    let (task_exit_tx, task_exit_rx) = tokio::sync::oneshot::channel::<()>();

    let shutdown = tokio_graceful::Shutdown::new(async move {
        tokio::select! {
            _ = task_exit_rx => {
                tracing::warn!("critical task exit observed, shutting down");
            }
            _ = tokio_graceful::default_signal() => {
                tracing::info!("external exit signal received, shutting down");
            }
        }
    });

    // Short for `shutdown.guard().into_spawn_task_fn(serve_tcp)`
    // In case you only wish to pass in a future (in contrast to a function)
    // as you do not care about being able to use the linked guard,
    // you can also use [`Shutdown::spawn_task`](https://docs.rs/tokio-graceful/latest/tokio_graceful/struct.Shutdown.html#method.spawn_task).
    let server_handle = shutdown.spawn_task_fn(serve_tcp);

    // spawn a task that will trigger a shutdown in case of an error with our server,
    // a common reason for this could be because you have an issue at server setup
    // (e.g. port that you try to bind to is already in use)
    let shutdown_err_guard = shutdown.guard_weak();
    tokio::spawn(async move {
        tokio::select! {
            _ = shutdown_err_guard.cancelled() => {
                // shutdown signal received, do nothing but exit this task
                return;
            }
            result = server_handle => {
                match result {
                    Ok(_) => {
                        tracing::info!("server exited, triggering manual shutdown");
                    }
                    Err(err) => {
                        tracing::error!(error = &err as &dyn std::error::Error, "server exited, triggering error shutdown");
                    }
                }
            }
        }
        task_exit_tx.send(()).unwrap();
    });

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
            tracing::warn!(
                error = &e as &dyn std::error::Error,
                "shutdown: forcefully due to timeout"
            );
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
        //
        // NOTE, make sure to pass a clone of the shutdown guard to the service fn
        // in case you wish to be able to cancel a long running process should the
        // shutdown signal be received and you know that your task might not finish on time.
        // This allows you to at least leave it behind in a consistent state such that another
        // process can pick up where you left that task.
        async { Ok::<_, Infallible>(service_fn(hello)) }
    });

    let addr = ([127, 0, 0, 1], 8080).into();

    let server = Server::bind(&addr).serve(make_svc);
    let server = server.with_graceful_shutdown(shutdown_guard.clone_weak().into_cancelled());

    if let Err(err) = server.await {
        tracing::error!(
            error = &err as &dyn std::error::Error,
            "server quit with error"
        );
    }
}

async fn hello(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    Ok(Response::new(Body::from("Hello World!")))
}
