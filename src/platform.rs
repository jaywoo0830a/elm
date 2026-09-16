//! Platform abstraction (사양서 7.1) — the platform is an argument.
//!
//! ```ignore
//! fn main() {
//!     elm_magic::run(Desktop, App);   // v0.3: Headless (+ egui adapter crate)
//! }
//! ```
//!
//! Components never know the platform; `run` injects it.

use crate::testing::TestApp;
use crate::Component;

/// A rendering target. `run(Headless, Comp)` returns a headless session
/// (`TestApp`) for tests; GUI platforms drive the render loop themselves.
pub trait Platform {
    /// What `run` hands back to the caller.
    type Session<C: Component>;
    fn run<C: Component>(self, _component: C) -> Self::Session<C>
    where
        C::Props: Default;
}

/// The test platform: mount once, no renderer, no runtime (사양서 8.1).
pub struct Headless;

impl Platform for Headless {
    type Session<C: Component> = TestApp<C>;

    fn run<C: Component>(self, _component: C) -> Self::Session<C>
    where
        C::Props: Default,
    {
        crate::testing::mount::<C>()
    }
}
