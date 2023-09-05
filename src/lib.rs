#![doc = include_str!("../README.md")]
#![warn(
    clippy::all,
    clippy::dbg_macro,
    clippy::todo,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::mem_forget,
    clippy::unused_self,
    clippy::filter_map_next,
    clippy::needless_continue,
    clippy::needless_borrow,
    clippy::match_wildcard_for_single_variants,
    clippy::if_let_mutex,
    clippy::mismatched_target_os,
    clippy::await_holding_lock,
    clippy::match_on_vec_items,
    clippy::imprecise_flops,
    clippy::suboptimal_flops,
    clippy::lossy_float_literal,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::fn_params_excessive_bools,
    clippy::exit,
    clippy::inefficient_to_string,
    clippy::linkedlist,
    clippy::macro_use_imports,
    clippy::option_option,
    clippy::verbose_file_reads,
    clippy::unnested_or_patterns,
    rust_2018_idioms,
    future_incompatible,
    nonstandard_style,
    missing_docs
)]
#![allow(elided_lifetimes_in_paths, clippy::type_complexity)]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]

mod guard;
pub use guard::{ShutdownGuard, WeakShutdownGuard};

mod shutdown;
pub use shutdown::{default_signal, Shutdown};

pub(crate) mod trigger;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

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

    #[tokio::test]
    async fn test_shutdown_guard_cancel_safety() {
        let (tx, rx) = oneshot::channel::<()>();
        let shutdown = Shutdown::new(async {
            rx.await.unwrap();
        });
        let guard = shutdown.guard();

        tokio::select! {
            _ = guard.cancelled() => {}
            _ = tokio::time::sleep(Duration::from_millis(50)) => {},
        }

        tx.send(()).unwrap();

        drop(guard);

        shutdown.shutdown().await;
    }

    #[tokio::test]
    async fn test_shutdown_guard_weak_cancel_safety() {
        let (tx, rx) = oneshot::channel::<()>();
        let shutdown = Shutdown::new(async {
            rx.await.unwrap();
        });
        let guard = shutdown.guard_weak();

        tokio::select! {
            _ = guard.into_cancelled() => {}
            _ = tokio::time::sleep(Duration::from_millis(50)) => {},
        }

        tx.send(()).unwrap();

        shutdown.shutdown().await;
    }

    #[tokio::test]
    async fn test_shutdown_cancelled_after_shutdown() {
        let (tx, rx) = oneshot::channel::<()>();
        let shutdown = Shutdown::new(async {
            rx.await.unwrap();
        });
        let weak_guard = shutdown.guard_weak();
        tx.send(()).unwrap();
        shutdown.shutdown().await;
        weak_guard.cancelled().await;
    }

    #[tokio::test]
    async fn test_shutdown_nested_guards() {
        let (tx, rx) = oneshot::channel::<()>();
        let shutdown = Shutdown::new(async {
            rx.await.unwrap();
        });
        shutdown.spawn_task_fn(|guard| async move {
            guard.spawn_task_fn(|guard| async move {
                guard.spawn_task_fn(|guard| async move {
                    guard.spawn_task(async {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    });
                });
            });
        });
        tx.send(()).unwrap();
        shutdown.shutdown().await;
    }

    #[tokio::test]
    async fn test_shutdown_sixten_thousand_guards() {
        let (tx, rx) = oneshot::channel::<()>();
        let shutdown = Shutdown::new(async {
            rx.await.unwrap();
        });
        for _ in 0..16_000 {
            shutdown.spawn_task(async {
                // sleep random between 1 and 80ms
                let duration = Duration::from_millis(rand::random::<u64>() % 80 + 1);
                tokio::time::sleep(duration).await;
            });
        }
        tx.send(()).unwrap();
        shutdown.shutdown().await;
    }
}
