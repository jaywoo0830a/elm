// v0.3 — <Raw> 탈출구 (사양서 7.3)
// <Raw>|ui: &mut PlatformType| { ... }</Raw> — 유일한 escape hatch.
// 헤드리스 테스트에서는 무시되고, 플랫폼 어댑터가 payload로 호출한다.

elm_magic::view! {
    fn Page() {
        <Col>
            <Raw>|ui: &mut String| { ui.push_str("egui-widget"); }</Raw>
            "hello"
        </Col>
    }
}

#[test]
fn raw_is_ignored_by_headless_text() {
    let app = elm_magic::mount!(Page);
    app.expect_text("hello");
    assert!(!app.text().contains("egui-widget"));
}

#[test]
fn raw_renders_as_placeholder() {
    let app = elm_magic::mount!(Page);
    assert!(
        app.render_tree().contains("[raw]"),
        "tree: {}",
        app.render_tree()
    );
}

#[test]
fn raw_can_be_invoked_with_platform_payload() {
    let app = elm_magic::mount!(Page);
    let mut out = String::new();
    // 어댑터가 하는 일: 트리를 돌며 Raw 위젯에 플랫폼 핸들(&mut egui::Ui 등)을 넘긴다
    elm_magic::raw::invoke(app.element(), &mut out);
    assert_eq!(out, "egui-widget");
}

#[test]
fn raw_never_leaks_into_buttons_or_inputs() {
    let app = elm_magic::mount!(Page);
    // Raw 안의 코드는 헤드리스 이벤트 탐색에 영향을 주지 않는다
    assert!(elm_magic::testing::find_button_text(app.element(), "+").is_none());
}
