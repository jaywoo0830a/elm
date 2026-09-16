//! # elm-magic
//!
//! **"상태는 변수, 이벤트는 대입, 화면은 함수 본문, 효과는 `<-`, 플랫폼은 인자."**
//!
//! 초경량 순수 함수형 UI 라이브러리. Rust의 타입 시스템을 유지하면서,
//! 매크로로 문법을 JS/JSX처럼 위장한다.
//!
//! ```ignore
//! use elm_magic::prelude::*;
//!
//! elm_magic::view! {
//!     fn Counter(n = 0) {
//!         <Row>
//!             <Button on_click={n -= 1}>"-"</Button>
//!             "Count: {n}"
//!             <Button on_click={n += 1}>"+"</Button>
//!         </Row>
//!     }
//! }
//!
//! #[test]
//! fn counter_increments() {
//!     let mut app = elm_magic::mount!(Counter);
//!     app.click("+");
//!     app.expect_text("Count: 1");
//! }
//! ```
//!
//! ## v0.1 스코프 (사양서 로드맵)
//!
//! - `view!` — 컴포넌트 정의 (매개변수 = 상태 슬롯)
//! - `ui!` — JSX 유사 요소 빌더 (Col, Row, Text, Button, Input)
//! - `mount!` / `mount_with` — 헤드리스 테스트 (렌더러·런타임 불필요)
//!
//! 참고: 매개변수 기본값(`n = 0`)은 유효한 Rust 문법이 아니므로
//! 속성 매크로(`#[view]`) 대신 함수형 매크로 `view! { fn ... }`로 정의한다.

mod element;
pub mod platform;
pub mod raw;
pub mod runtime;
pub mod style;
mod state;
pub mod testing;

/// Register an effect mock on a mounted app (사양서 8.2 — `.mock()` 슈가).
///
/// ```ignore
/// let mut app = elm_magic::mount!(Search);
/// elm_magic::mock!(app, search_api, |q: String| vec![format!("hit:{}", q)]);
/// ```
///
/// The parameter type annotations give the registry its downcast keys.
#[macro_export]
macro_rules! mock {
    ($app:ident, $name:ident, |$a:ident : $ta:ty| $body:expr) => {
        $app.set_mock1(stringify!($name), move |$a: $ta| $body)
    };
    ($app:ident, $name:ident, |$a:ident : $ta:ty, $b:ident : $tb:ty| $body:expr) => {
        $app.set_mock2(stringify!($name), move |$a: $ta, $b: $tb| $body)
    };
    ($app:ident, $name:ident, |$a:ident : $ta:ty, $b:ident : $tb:ty, $c:ident : $tc:ty| $body:expr) => {
        $app.set_mock3(stringify!($name), move |$a: $ta, $b: $tb, $c: $tc| $body)
    };
}

pub use element::{Element, IntoElement, IntoElements};
pub use state::{Arena, Ctx, State};

/// 뷰 본문을 `Element`로 마감한다 (본문이 요소 목록이면 `Fragment`로 감쌈).
pub fn into_element<T: IntoElement>(value: T) -> Element {
    value.into_element()
}

/// A component: pure function from state to an element tree.
pub trait Component {
    type Props: Default;
    /// Number of state slots this component owns.
    const SLOTS: usize;
    fn render(ctx: &mut Ctx, props: &Self::Props) -> Element;
}

/// Re-exported procedural macros.
pub use elm_magic_macros::{css, ui, view};

/// Mount a component headlessly (no renderer, no runtime) for tests.
pub use testing::{mount, mount_with};
pub use platform::{Headless, Platform};

/// Entry point (사양서 7.1): the platform is an argument, the component
/// never knows it.
pub fn run<P: platform::Platform, C: Component>(
    platform: P,
    component: C,
) -> P::Session<C>
where
    C::Props: Default,
{
    platform.run(component)
}

pub mod prelude {
    pub use crate::element::{Element, IntoElement, IntoElements};
    pub use crate::state::{Arena, Ctx, State};
    pub use crate::testing::{mount, mount_with, TestApp};
    pub use crate::platform::{Headless, Platform};
    pub use crate::{into_element, run};
    pub use crate::Component;
    pub use crate::style;
    pub use elm_magic_macros::{css, ui, view};
}

/// `mount!(Counter)` — headless mount. `mount!(Counter, props)` — with props.
#[macro_export]
macro_rules! mount {
    ($t:ty) => {
        $crate::testing::mount::<$t>()
    };
    ($t:ty, $props:expr) => {
        $crate::testing::mount_with::<$t>($props)
    };
}

