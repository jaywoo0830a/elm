//! Minimal effect runtime (v0.2).
//!
//! `block_on` polls a future to completion with a noop waker. In headless
//! tests, effect futures are plain `async` blocks that complete without
//! real I/O, so the first poll(s) suffice. `Pin`/`Box`/`Send`/`'static`
//! are never exposed to user code (사양서 5.1).

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = Box::pin(fut);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(&waker);
    loop {
        match Pin::new(&mut fut).poll(&mut cx) {
            Poll::Ready(out) => return out,
            Poll::Pending => {
                // No real reactor in headless mode: spin. Async blocks that
                // only transform data become Ready on a later poll.
                std::thread::yield_now();
            }
        }
    }
}
