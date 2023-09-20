//! An example showcasing how to use [`tokio_graceful`] to
//! await ongoing tasks before shutting down.
//!
//! Some people know this concept better as Waitgroup.
//!
//! In languages like Golang this is a very common pattern
//! for waiting for all tasks to finish before shutting down,
//! and is even part of their standard library.
//!
//! [`tokio_graceful`]: https://docs.rs/tokio-graceful

use std::time::Duration;

use tokio_graceful::Shutdown;

#[tokio::main]
async fn main() {
    let shutdown = Shutdown::no_signal();

    const MAX: u64 = 5;

    for countdown in 0..=MAX {
        // NOTE: you can also manually create
        // a guard using `shutdown.guard()` and spawn
        // you async tasks manually in case you do not wish to run these
        // using `tokio_graceful::sync::spawn` (Tokio by default).
        let sleep = tokio::time::sleep(Duration::from_secs(countdown));
        shutdown.spawn_task(async move {
            sleep.await;
            if countdown == MAX {
                println!("Go!");
            } else {
                println!("{}...", MAX - countdown);
            }
        });
    }

    shutdown.shutdown().await;
    println!("Success :)");
}
