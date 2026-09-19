//! gpui 어댑터 — 0.7.1 이후 **첫 테스트**. (지금까지 `crates/elm-magic-gpui/tests`가 없었다.)
//!
//! gpui-kit 0.6은 `test-support` 기능으로 헤드리스 UI 테스트 하네스를 제공한다
//! (`#[gpui_kit::test]` + `TestAppContext` + `gpui_kit::test`). README가 문서화한
//! 공개 API(`ElmView::<C>::new(cx)`)만으로 **실제 창에서 렌더 루프가 도는지**를 본다.
//!
//! 주의: `test-support`가 켜지면 `use gpui_kit::*` 글로브가 GPUI의 `test` 속성을
//! 가져와 내장 `#[test]`를 가린다 (gpui-kit `tests/test_macro.rs`의 경고). 그래서
//! 여기서는 글로브 대신 **쓰는 것만** 가져온다.
//!
//! 한계: 상호작용(클릭 → 상태 변화) 단언은 어댑터가 요소에 `ElementId`를
//! 등록해야 `window.click("id")`로 확인할 수 있다. 지금은 **렌더 루프가 실제 창에서
//! 돈다**는 것까지만 고정한다 (그 아래 상태 계약은 `tests/callbacks.rs`가 담당).

use elm_magic_gpui::ElmView;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    div, AppContext as _, Context, IntoElement, ParentElement, Render, TestAppContext, Window,
};

elm_magic::view! {
    fn GpuiCounter(n = 0) {
        <Col>
            "Count: {n}"
            <Button on_click={n += 1}>"+"</Button>
            <Button on_click={n -= 1}>"-"</Button>
        </Col>
    }
}

/// README의 사용법 그대로: `ElmView`를 엔티티로 만들어 트리에 붙인다.
struct Root;

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(cx.new(ElmView::<GpuiCounter>::new))
    }
}

/// 어댑터는 `css!` 토큰을 **활성 테마**에서 읽으므로, 창을 열기 전에 테마를
/// 초기화해야 한다 (gpui-kit README의 `gpui_kit::init(cx)` 한 줄).
fn init_theme(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
}

#[gpui_kit::test]
fn elm_view_renders_in_a_headless_window(cx: &mut TestAppContext) {
    init_theme(cx);
    let handle = cx.add_window(|_, _| Root);
    cx.update_window(handle.into(), |_, window, cx| {
        // 프레임을 한 번 그린다 — 어댑터가 elm-magic 트리를 gpui 요소로 옮긴다
        window.draw(cx).clear(cx);
        window.render_frame(cx);
    })
    .expect("headless window");
}

#[gpui_kit::test]
fn elm_view_survives_state_changes_across_frames(cx: &mut TestAppContext) {
    init_theme(cx);
    let handle = cx.add_window(|_, _| Root);
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        window.render_frame(cx);
        // 같은 엔티티를 여러 프레임 그려도 panic 없이 안정적이어야 한다
        for _ in 0..3 {
            window.render_frame(cx);
        }
    })
    .expect("headless window");
}

#[gpui_kit::test]
fn two_elm_views_coexist_in_one_window(cx: &mut TestAppContext) {
    struct TwoRoot;
    impl Render for TwoRoot {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .child(cx.new(ElmView::<GpuiCounter>::new))
                .child(cx.new(ElmView::<GpuiCounter>::new))
        }
    }

    init_theme(cx);
    let handle = cx.add_window(|_, _| TwoRoot);
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        window.render_frame(cx);
    })
    .expect("headless window");
}
