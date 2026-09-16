//! State — arena slots. `State<T>` is a `Copy` index handle.

use std::any::Any;
use std::marker::PhantomData;

/// Slot storage. All component state lives here; nothing else is mutated.
#[derive(Default)]
pub struct Arena {
    slots: Vec<Option<Box<dyn Any>>>,
}

impl Arena {
    pub fn new() -> Self {
        Self::default()
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
/// components get disjoint slots.
pub struct Ctx {
    pub arena: Arena,
    pub base: usize,
}

impl Ctx {
    pub fn new() -> Self {
        Ctx { arena: Arena::new(), base: 0 }
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
