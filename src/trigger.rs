//! A trigger is a way to wake up a task from another task.
//!
//! This is useful for implementing graceful shutdowns, among other things.
//! The way it works is a Sender and Receiver both have access to shared data,
//! being a WakerList and a boolean indicating whether the trigger has been triggered.
//!
//! The Sender can trigger the Receiver by setting the boolean to true and waking up all the wakers.
//! The Receiver can add itself to the waker list (when being polled) and check whether the boolean
//! has been set to true.
//!
//! Using Arc, Mutex and Atomic* this is all done in a safe manner.
//! The trick is further to use Slab to store the wakers, as it allows
//! us to very efficiently keep track of the wakers and remove them when they are no longer needed.
//!
//! To make this work, in a cancel safe manner, we need to make sure
//! we remove the waker from the waker list when the Receiver is dropped.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use pin_project_lite::pin_project;
use slab::Slab;

use crate::sync::{Arc, AtomicBool, Mutex, Ordering};

type WakerList = Arc<Mutex<Slab<Option<Waker>>>>;
type TriggerState = Arc<AtomicBool>;

/// A subscriber is the active state of a Receiver,
/// and is there only when the Receiver did not yet detect a trigger.
#[derive(Debug, Clone)]
struct Subscriber {
    wakers: WakerList,
    state: TriggerState,
}

/// The state of a [`Subscriber] returned by `Subscriber::state`,
/// which is used to determine whether the Subscriber has been triggered
/// or has instead stored the callee's `Waker` for being able to wake it up
/// when the trigger is triggered.
#[derive(Debug)]
enum SubscriberState {
    Waiting(usize),
    Triggered,
}

impl Subscriber {
    /// Returns the state of the Subscriber,
    /// which is used as a main driver in the Receiver's `Future::poll` implementation.
    ///
    /// If the Subscriber has been triggered, it returns `SubscriberState::Triggered`.
    /// If the Subscriber has not yet been triggered, it returns `SubscriberState::Waiting`
    /// with the key of the waker in the waker list.
    ///
    /// If the key is `Some`, it means the waker is already in the waker list,
    /// and we can update the waker with the new waker. Otherwise we insert the waker
    /// into the waker list as a new waker. Either way, we return the key of the waker.
    pub fn state(&self, cx: &mut Context, key: Option<usize>) -> SubscriberState {
        if self.state.load(Ordering::SeqCst) {
            return SubscriberState::Triggered;
        }

        let mut wakers = self.wakers.lock().unwrap();

        let waker = Some(cx.waker().clone());

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

/// The state of a [`Receiver`], which is either open or closed.
/// The closed state is mostly for simplification and optimization reasons.
///
/// When the Receiver is open, it contains a [`Subscriber`],
/// which is used to determine whether the Receiver has been triggered.
#[derive(Debug)]
enum ReceiverState {
    Open { sub: Subscriber, key: Option<usize> },
    Closed,
}

impl Clone for ReceiverState {
    /// Clone either nothing or the [`Subscriber`].
    /// Very important however to not clone its key as
    /// that is linked to a polled future of the original Receiver,
    /// and not the cloned one.
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
    /// When the Receiver is dropped, we need to remove the waker from the waker list.
    /// As to ensure the Receiver is cancel safe.
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

    /// Polls the Receiver, which is either open or closed.
    ///
    /// When the Receiver is open, it uses the [`Subscriber`] to determine
    /// whether the Receiver has been triggered.
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
    state: TriggerState,
    wakers: WakerList,
}

impl Sender {
    fn new(wakers: WakerList, state: TriggerState) -> Self {
        Self { wakers, state }
    }

    /// Triggers the Receiver, with a short circuit if the trigger has already been triggered.
    pub fn trigger(&self) {
        if self.state.swap(true, Ordering::SeqCst) {
            return;
        }

        let mut wakers = self.wakers.lock().unwrap();
        for (key, waker) in wakers.iter_mut() {
            match waker.take() {
                Some(waker) => {
                    tracing::trace!("trigger::Sender: wake up waker with key: {}", key);
                    waker.wake();
                }
                None => {
                    tracing::trace!(
                        "trigger::Sender: nop: waker already triggered with key: {}",
                        key
                    );
                }
            }
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
