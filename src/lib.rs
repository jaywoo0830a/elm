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
mod state;
pub mod testing;

pub use element::{Element, IntoElements};
pub use state::{Arena, Ctx, State};

/// A component: pure function from state to an element tree.
pub trait Component {
    type Props: Default;
    /// Number of state slots this component owns.
    const SLOTS: usize;
    fn render(ctx: &mut Ctx, props: &Self::Props) -> Element;
}

/// Re-exported procedural macros.
pub use elm_magic_macros::{ui, view};

/// Mount a component headlessly (no renderer, no runtime) for tests.
pub use testing::{mount, mount_with};

pub mod prelude {
    pub use crate::element::{Element, IntoElements};
    pub use crate::state::{Arena, Ctx, State};
    pub use crate::testing::{mount, mount_with, TestApp};
    pub use crate::Component;
    pub use elm_magic_macros::{ui, view};
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

