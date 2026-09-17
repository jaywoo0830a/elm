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

// ── mock (사양서 8.2) ────────────────────────────────────────
//
// Effect functions (`f(args)` in `<-` position) are dispatched through a
// thread-local registry. Tests replace them with `mock!`; the real function
// is constructed but never polled when a mock is registered.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ── 스트림 목 (사양서 8.2 — 스트림도 교체 가능) ─────────────

thread_local! {
    /// 스트림 목: 이름 → 값 목록 (타입 소거). `mock_stream!`이 등록한다.
    static STREAM_MOCKS: RefCell<HashMap<String, Vec<Box<dyn Any>>>> = RefCell::new(HashMap::new());
}

/// Register a stream mock: `mock_stream!(app, ws, [msg1, msg2])`.
pub fn set_stream_mock<T: 'static>(name: &str, values: Vec<T>) {
    STREAM_MOCKS.with(|m| {
        m.borrow_mut().insert(
            name.to_string(),
            values.into_iter().map(|v| Box::new(v) as Box<dyn Any>).collect(),
        );
    });
}

/// Take (and consume) a registered stream mock, downcasting to `T`.
pub fn take_stream_mock<T: 'static>(name: &str) -> Option<Vec<T>> {
    STREAM_MOCKS.with(|m| {
        m.borrow_mut().remove(name).map(|values| {
            values
                .into_iter()
                .map(|v| {
                    *v.downcast::<T>()
                        .unwrap_or_else(|_| panic!("elm-magic stream mock: value type mismatch for {:?}", name))
                })
                .collect()
        })
    })
}

type MockFn = Rc<dyn Fn(&[&dyn Any]) -> Box<dyn Any>>;

thread_local! {
    static MOCKS: RefCell<HashMap<String, MockFn>> = RefCell::new(HashMap::new());
}

/// Register a mock under `name`, and also under its short (last path segment)
/// name so that `.mock(search_api, ..)` (which recovers the full type name)
/// and `mock!(app, search_api, ..)` (which uses `stringify!`) hit the same entry.
fn insert_mock(name: &str, f: MockFn) {
    MOCKS.with(|m| {
        let mut map = m.borrow_mut();
        map.insert(name.to_string(), Rc::clone(&f));
        if let Some(tail) = name.rsplit("::").next() {
            if tail != name {
                map.insert(tail.to_string(), f);
            }
        }
    });
}

/// Register a 1-arg mock. Call via `mock!(app, name, |a: T| ...)` — the
/// argument type annotation gives the registry its downcast key.
pub fn set_mock1<A: Clone + 'static, Out: 'static>(name: &str, f: impl Fn(A) -> Out + 'static) {
    let name = name.to_string();
    let mock_name = name.clone();
    let f: MockFn = Rc::new(move |args: &[&dyn Any]| {
                let a: &A = args
                    .first()
                    .and_then(|v| v.downcast_ref())
                    .unwrap_or_else(|| panic!("elm-magic mock: argument type mismatch for {:?}", mock_name));
                Box::new(f(a.clone())) as Box<dyn Any>
            });
    insert_mock(&name, f);
}

/// Register a 2-arg mock.
pub fn set_mock2<A0: Clone + 'static, A1: Clone + 'static, Out: 'static>(
    name: &str,
    f: impl Fn(A0, A1) -> Out + 'static,
) {
    let name = name.to_string();
    let mock_name = name.clone();
    let f: MockFn = Rc::new(move |args: &[&dyn Any]| {
                let a0: &A0 = args
                    .first()
                    .and_then(|v| v.downcast_ref())
                    .unwrap_or_else(|| panic!("elm-magic mock: argument type mismatch for {:?}", mock_name));
                let a1: &A1 = args
                    .get(1)
                    .and_then(|v| v.downcast_ref())
                    .unwrap_or_else(|| panic!("elm-magic mock: argument type mismatch for {:?}", mock_name));
                Box::new(f(a0.clone(), a1.clone())) as Box<dyn Any>
            });
    insert_mock(&name, f);
}

/// Register a 3-arg mock.
pub fn set_mock3<A0: Clone + 'static, A1: Clone + 'static, A2: Clone + 'static, Out: 'static>(
    name: &str,
    f: impl Fn(A0, A1, A2) -> Out + 'static,
) {
    let name = name.to_string();
    let mock_name = name.clone();
    let f: MockFn = Rc::new(move |args: &[&dyn Any]| {
                let a0: &A0 = args
                    .first()
                    .and_then(|v| v.downcast_ref())
                    .unwrap_or_else(|| panic!("elm-magic mock: argument type mismatch for {:?}", mock_name));
                let a1: &A1 = args
                    .get(1)
                    .and_then(|v| v.downcast_ref())
                    .unwrap_or_else(|| panic!("elm-magic mock: argument type mismatch for {:?}", mock_name));
                let a2: &A2 = args
                    .get(2)
                    .and_then(|v| v.downcast_ref())
                    .unwrap_or_else(|| panic!("elm-magic mock: argument type mismatch for {:?}", mock_name));
                Box::new(f(a0.clone(), a1.clone(), a2.clone())) as Box<dyn Any>
            });
    insert_mock(&name, f);
}

/// Invoke a registered mock with type-erased args; None if not registered.
pub fn call_mock<Out: 'static>(name: &str, args: Vec<&dyn Any>) -> Option<Out> {
    MOCKS.with(|m| {
        let m = m.borrow();
        match m.get(name) {
            Some(f) => {
                let out = f(&args)
                    .downcast::<Out>()
                    .unwrap_or_else(|_| panic!("elm-magic mock: return type mismatch for {:?}", name));
                Some(*out)
            }
            None => None,
        }
    })
}

/// Is a mock registered for this effect fn?
pub fn has_mock(name: &str) -> bool {
    MOCKS.with(|m| m.borrow().contains_key(name))
}

/// Run the mock for `name` if registered, otherwise the real future.
/// The real future is constructed but never polled when mocked.
pub fn maybe_mock<F, M>(name: &str, real: F, mock: M) -> Pin<Box<dyn Future<Output = F::Output>>>
where
    F: Future + 'static,
    F::Output: 'static,
    M: FnOnce() -> Option<F::Output>,
{
    if has_mock(name) {
        if let Some(out) = mock() {
            return Box::pin(std::future::ready(out));
        }
    }
    Box::pin(real)
}

/// Drop all registered mocks (per-thread).
pub fn clear_mocks() {
    MOCKS.with(|m| m.borrow_mut().clear());
}
