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

    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

    let token = tokio_graceful::token();
    tokio::pin!(token);

    tracing::info!("listening on {}", listener.local_addr().unwrap());
    loop {
        tokio::select! {
            result = token.shutdown_with_delay(Duration::from_secs(30)) => {
                match result {
                    Ok(_) => {
                        tracing::info!("shutting down gracefully");
                    }
                    Err(e) => {
                        tracing::warn!("shutdown error: {:?}", e);
                    }
                }
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((socket, _)) => {
                        let token = token.token();
                        tokio::spawn(async move {
                            let _ = token;
                            let (mut reader, mut writer) = tokio::io::split(socket);
                            tokio::io::copy(&mut reader, &mut writer).await.unwrap();
                        });
                    }
                    Err(e) => {
                        tracing::warn!("accept error: {:?}", e);
                    }
                }
            }
        }
    }

    tracing::info!("Bye!");
}
