//! State — arena slots. `State<T>` is a `Copy` index handle.

use std::any::Any;
use std::future::Future;
use std::marker::PhantomData;
use std::rc::Rc;

/// A deferred effect continuation: runs with the arena once its future resolves.
pub type PendingEffect = Box<dyn FnOnce(&mut Arena)>;

/// A live stream task: pumps one value; returns false when exhausted.
pub type StreamTask = Box<dyn FnMut(&mut Arena) -> bool>;

/// Keyboard handler registered by `on_key` (사양서 5.3).
pub type KeyHandler = Rc<dyn Fn(&mut Arena)>;

/// A queued effect with its due time (ms on the simulated clock).
struct Pending {
    due: u64,
    run: PendingEffect,
}

/// Slot storage. All component state lives here; nothing else is mutated.
#[derive(Default)]
pub struct Arena {
    slots: Vec<Option<Box<dyn Any>>>,
    /// Effects spawned by `<-` — run by `flush()` (사양서 12: Cmd는 flush 전까지 실행 안 됨).
    /// `<- f() after 300ms`는 `due`가 미래라 `advance(ms)`가 지나야 실행된다.
    pending: Vec<Pending>,
    /// Simulated clock in ms (사양서 8.2). `advance`가 전진시킨다.
    now: u64,
    /// Live streams registered by `->` — pumped by `pump()` (사양서 5.4).
    streams: Vec<StreamTask>,
}

impl Arena {
    pub fn new() -> Self {
        Self::default()
    }

    /// Simulated clock (ms).
    pub fn now(&self) -> u64 {
        self.now
    }

    /// Set the simulated clock (used by `TestApp::advance`).
    pub fn set_now(&mut self, t: u64) {
        self.now = t;
    }

    fn ensure_len(&mut self, idx: usize) {
        if self.slots.len() <= idx {
            self.slots.resize_with(idx + 1, || None);
        }
    }

    pub fn slot<T: 'static>(&mut self, idx: usize, init: impl FnOnce() -> T) -> State<T> {
        self.ensure_len(idx);
        if self.slots[idx].is_none() {
            self.slots[idx] = Some(Box::new(init()));
        }
        State { idx, _pd: PhantomData }
    }

    pub fn get<T: 'static>(&self, idx: usize) -> &T {
        self.slots[idx]
            .as_ref()
            .expect("slot not initialized")
            .downcast_ref::<T>()
            .expect("slot type mismatch")
    }

    pub fn set<T: 'static>(&mut self, idx: usize, value: T) {
        self.slots[idx] = Some(Box::new(value));
    }

    pub fn mutate<T: 'static, F: FnOnce(&mut T)>(&mut self, idx: usize, f: F) {
        let v = self.get_mut::<T>(idx);
        f(v);
    }

    fn get_mut<T: 'static>(&mut self, idx: usize) -> &mut T {
        self.slots[idx]
            .as_mut()
            .expect("slot not initialized")
            .downcast_mut::<T>()
            .expect("slot type mismatch")
    }

    /// Spawn an effect: poll the future to completion, then run the
    /// continuation with the arena (사양서 5.1 — 런타임이 스폰·폴링·재디스패치).
    /// The effect is due immediately (`flush()`가 실행).
    pub fn spawn<F, G>(&mut self, fut: F, k: G)
    where
        F: Future + 'static,
        F::Output: 'static,
        G: FnOnce(&mut Arena, F::Output) + 'static,
    {
        self.spawn_after(0, fut, k);
    }

    /// Spawn a delayed effect: `targets <- f() after 300ms` (사양서 5.2 —
    /// `after` 접미사). `flush()`는 아직 due가 아닌 효과를 실행하지 않고,
    /// `advance(ms)`가 시계를 전진시켜 실행한다.
    pub fn spawn_after<F, G>(&mut self, delay_ms: u64, fut: F, k: G)
    where
        F: Future + 'static,
        F::Output: 'static,
        G: FnOnce(&mut Arena, F::Output) + 'static,
    {
        let due = self.now + delay_ms;
        self.pending.push(Pending {
            due,
            run: Box::new(move |arena| {
                let out = crate::runtime::block_on(fut);
                k(arena, out);
            }),
        });
    }

    /// Number of pending effects (due or not).
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Take every pending effect, regardless of due time.
    pub fn take_pending(&mut self) -> Vec<PendingEffect> {
        std::mem::take(&mut self.pending)
            .into_iter()
            .map(|p| p.run)
            .collect()
    }

    /// Take only the effects whose due time has arrived, keeping the rest
    /// queued. This is what `flush()` and `advance()` call.
    pub fn take_due(&mut self) -> Vec<PendingEffect> {
        let now = self.now;
        let mut due = Vec::new();
        let mut rest = Vec::new();
        for p in std::mem::take(&mut self.pending) {
            if p.due <= now {
                due.push(p.run);
            } else {
                rest.push(p);
            }
        }
        self.pending = rest;
        due
    }

    /// Is an effect queued for later (`<- f() after 300ms`)?
    pub fn has_deferred(&self) -> bool {
        self.pending.iter().any(|p| p.due > self.now)
    }

    /// Register a stream: each yielded value runs the continuation with the
    /// arena (사양서 5.4 — 스트림을 슬롯에 반영).
    pub fn spawn_stream<I, G>(&mut self, iter: I, mut k: G)
    where
        I: Iterator + 'static,
        I::Item: 'static,
        G: FnMut(&mut Arena, I::Item) + 'static,
    {
        let mut it = iter;
        self.streams.push(Box::new(move |arena| match it.next() {
            Some(v) => {
                k(arena, v);
                true
            }
            None => false,
        }));
    }

    /// Number of live stream tasks.
    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    /// Take all live stream tasks (used by `pump`).
    pub fn take_streams(&mut self) -> Vec<StreamTask> {
        std::mem::take(&mut self.streams)
    }

    /// Put stream tasks back (survivors of a pump round).
    pub fn push_streams(&mut self, tasks: Vec<StreamTask>) {
        self.streams.extend(tasks);
    }
}

/// Copy handle to a state slot of type `T`.
pub struct State<T> {
    idx: usize,
    _pd: PhantomData<fn() -> T>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for State<T> {}

impl<T: 'static> State<T> {
    pub fn get<'a>(&'a self, arena: &'a Arena) -> &'a T {
        arena.get(self.idx)
    }
    pub fn set(&self, arena: &mut Arena, value: T) {
        arena.set(self.idx, value);
    }
    pub fn mutate<F: FnOnce(&mut T)>(&self, arena: &mut Arena, f: F) {
        arena.mutate(self.idx, f);
    }
}

/// Render context: the arena plus a slot-index base offset so nested
/// components get disjoint slots, the simulated clock, and key handlers.
pub struct Ctx {
    pub arena: Arena,
    pub base: usize,
    /// Simulated clock in ms — advanced by `TestApp::advance` (사양서 8.2).
    pub now: u64,
    /// Keyboard handlers registered by `on_key` during the last render.
    pub keys: Vec<(String, KeyHandler)>,
}

impl Ctx {
    pub fn new() -> Self {
        Ctx { arena: Arena::new(), base: 0, now: 0, keys: Vec::new() }
    }

    pub fn slot<T: 'static>(&mut self, idx: usize, init: impl FnOnce() -> T) -> State<T> {
        self.arena.slot(self.base + idx, init)
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}
