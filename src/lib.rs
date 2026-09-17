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

/// Register a stream mock: `mock_stream!(app, ws, [msg1, msg2])` (사양서 8.2).
///
/// 스트림 소스(`f(args)`)가 목과 같은 이름이면 목의 값들이 순서대로 흘러간다.
#[macro_export]
macro_rules! mock_stream {
    ($app:ident, $name:ident, [$($v:expr),* $(,)?]) => {
        $app.set_stream_mock(stringify!($name), ::std::vec![$($v),*])
    };
}

pub use element::{Callback, Element, IntoElement, IntoElements};
pub use state::{Arena, Ctx, FrameTail, State};

/// 이벤트/콜백으로 들어온 값(`_`)을 복제한다.
///
/// `(_elm_v.clone())` 대신 이 함수를 거치면 타입 추론이 잘 된다 —
/// 컴포넌트 콜백 prop(`on_select: fn(Id)`)의 인자 타입은 호출부에서
/// 추론되어야 하기 때문 (사양서 3.1).
pub fn clone_value<T: Clone>(value: &T) -> T {
    value.clone()
}

/// `on_navigate` 핸들러가 받은 타입 소거 값을 꺼낸다 (사양서 5.3).
///
/// 타입은 본문 사용처에서 추론된다: `on_navigate(|r| route = r)` → `r: Route`.
/// 런타임에 넣은 값과 타입이 다르면 panic.
pub fn nav_take<T: Clone + 'static>(value: &dyn std::any::Any) -> T {
    value
        .downcast_ref::<T>()
        .unwrap_or_else(|| panic!("elm-magic: navigate 값의 타입이 맞지 않습니다"))
        .clone()
}

/// `key={expr}` → 인스턴스 경로에 쓸 안정적인 문자열 (사양서 9.5).
pub fn key_of<T: std::fmt::Debug>(value: &T) -> String {
    format!("{:?}", value)
}

/// 뷰 본문을 `Element`로 마감한다 (본문이 요소 목록이면 `Fragment`로 감쌈).
pub fn into_element<T: IntoElement>(value: T) -> Element {
    value.into_element()
}

/// 한 프레임 렌더 (사양서 9.5, 5.3):
/// 렌더 → (사라진 키의 `on_unmount` + 대기 중인 `on_event` / `on_net_change` /
/// `on_navigate` 전달) → 변화가 있었으면 다시 렌더.
///
/// keyed 슬롯 덕분에 사라진 인스턴스의 상태는 초기화된다.
pub fn frame<C: Component>(ctx: &mut Ctx, props: &C::Props) -> Element {
    ctx.begin_frame();
    let mut tree = C::render(ctx, props);
    for _ in 0..64 {
        let tail = ctx.end_frame();
        if tail.is_empty() {
            break;
        }
        tail.run(&mut ctx.arena);
        ctx.begin_frame();
        tree = C::render(ctx, props);
    }
    tree
}

/// A component: pure function from state to an element tree.
pub trait Component {
    type Props: Default;
    /// Number of state slots this component owns.
    const SLOTS: usize;
    fn render(ctx: &mut Ctx, props: &Self::Props) -> Element;
}

/// Re-exported procedural macros.
pub use elm_magic_macros::{css, store, store_fn, ui, view};

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
    pub use crate::element::{Callback, Element, IntoElement, IntoElements};
    pub use crate::state::{Arena, Ctx, FrameTail, State};
    pub use crate::testing::{mount, mount_with, TestApp};
    pub use crate::platform::{Headless, Platform};
    pub use crate::{clone_value, frame, into_element, key_of, mock, mock_stream, nav_take, run};
    pub use crate::Component;
    pub use crate::style;
    pub use elm_magic_macros::{css, store, store_fn, ui, view};
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

