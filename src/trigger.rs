use std::{
    future::Future,
    pin::Pin,
    sync::{atomic::AtomicBool, Arc, Mutex},
    task::{Context, Poll, Waker},
};

use pin_project_lite::pin_project;
use slab::Slab;

type WakerList = Arc<Mutex<Slab<Waker>>>;
type TriggerState = Arc<AtomicBool>;

#[derive(Debug, Clone)]
struct Subscriber {
    wakers: WakerList,
    state: TriggerState,
}

#[derive(Debug)]
enum SubscriberState {
    Waiting(usize),
    Triggered,
}

impl Subscriber {
    pub fn state(&self, cx: &mut Context, key: Option<usize>) -> SubscriberState {
        if self.state.load(std::sync::atomic::Ordering::SeqCst) {
            return SubscriberState::Triggered;
        }

        let mut wakers = self.wakers.lock().unwrap();
        if self.state.load(std::sync::atomic::Ordering::SeqCst) {
            return SubscriberState::Triggered;
        }

        let waker = cx.waker().clone();

        SubscriberState::Waiting(if let Some(key) = key {
            tracing::trace!("trigger::Subscriber: updating waker for key: {}", key);
            *wakers.get_mut(key).unwrap() = waker;
            key
        } else {
            let key = wakers.insert(waker);
            tracing::trace!("trigger::Subscriber: insert waker for key: {}", key);
            key
        })
    }
}

#[derive(Debug)]
enum ReceiverState {
    Open { sub: Subscriber, key: Option<usize> },
    Closed,
}

impl Clone for ReceiverState {
    fn clone(&self) -> Self {
        match self {
            ReceiverState::Open { sub, .. } => ReceiverState::Open {
                sub: sub.clone(),
                key: None,
            },
            ReceiverState::Closed => ReceiverState::Closed,
        }
    }
}

impl Drop for ReceiverState {
    fn drop(&mut self) {
        if let ReceiverState::Open { sub, key } = self {
            if let Some(key) = key.take() {
                let mut wakers = sub.wakers.lock().unwrap();
                tracing::trace!(
                    "trigger::ReceiverState::Drop: remove waker for key: {}",
                    key
                );
                wakers.remove(key);
            }
        }
    }
}

pin_project! {
    #[derive(Debug, Clone)]
    pub struct Receiver {
        state: ReceiverState,
    }
}

impl Receiver {
    fn new(wakers: WakerList, state: TriggerState) -> Self {
        Self {
            state: ReceiverState::Open {
                sub: Subscriber { wakers, state },
                key: None,
            },
        }
    }
}

impl Future for Receiver {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = self.project();
        match this.state {
            ReceiverState::Open { sub, key } => {
                let state = sub.state(cx, *key);
                match state {
                    SubscriberState::Waiting(new_key) => {
                        *key = Some(new_key);
                        std::task::Poll::Pending
                    }
                    SubscriberState::Triggered => {
                        *this.state = ReceiverState::Closed;
                        std::task::Poll::Ready(())
                    }
                }
            }
            ReceiverState::Closed => std::task::Poll::Ready(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sender {
    wakers: WakerList,
    state: TriggerState,
}

impl Sender {
    fn new(wakers: WakerList, state: TriggerState) -> Self {
        Self { wakers, state }
    }

    pub fn trigger(&self) {
        let wakers = self.wakers.lock().unwrap();
        self.state.store(true, std::sync::atomic::Ordering::SeqCst);
        for (key, waker) in wakers.iter() {
            tracing::trace!("trigger::Sender: wake up waker with key: {}", key);
            waker.wake_by_ref();
        }
    }
}

pub fn trigger() -> (Sender, Receiver) {
    let wakers = Arc::new(Mutex::new(Slab::new()));
    let state = Arc::new(AtomicBool::new(false));

    let sender = Sender::new(wakers.clone(), state.clone());
    let receiver = Receiver::new(wakers, state);

    (sender, receiver)
}
