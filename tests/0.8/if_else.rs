//! 0.8.0 조건 태그 — `<If>` / `<Else>` (사양: `0.8-preview.md` §3.1).
//!
//! 계약:
//! - `when`이 참이면 `<If>` 자식, 거짓이면 `<Else>` 자식.
//! - `<Else>`가 없으면 거짓일 때 **아무것도 그리지 않는다**.
//! - `<If>`/`<Else>`는 레이아웃 노드를 만들지 않는다 (자식만 그대로 나온다).
//! - 중첩, 동적 상태 변화, 본문 전체 분기를 모두 지원한다.
//!
//! 0.8.0은 조건/반복만 구현한다. 이 파일은 `cargo test --test v0_8_if_else`로 돈다.

// ── 참/거짓 두 분기 ─────────────────────────────────────────
elm_magic::view! {
    fn Greeting(n = 0) {
        <Col>
            <If when={n > 0}>
                <Text>"then-branch"</Text>
            <Else>
                <Text>"else-branch"</Text>
            </Else>
            </If>
        </Col>
    }
}

#[test]
fn if_true_renders_then_branch() {
    let app = elm_magic::mount_with::<Greeting>(GreetingProps {
        n: Some(5),
        ..Default::default()
    });
    app.expect_text("then-branch");
    app.assert_hidden("else-branch");
}

#[test]
fn if_false_renders_else_branch() {
    let app = elm_magic::mount!(Greeting);
    app.expect_text("else-branch");
    app.assert_hidden("then-branch");
}

// ── `<Else>` 생략 ───────────────────────────────────────────
elm_magic::view! {
    fn Optional(visible = false) {
        <Col>
            <If when={visible}>
                <Text>"shown"</Text>
            </If>
            <Text>"always"</Text>
        </Col>
    }
}

#[test]
fn if_without_else_renders_nothing_when_false() {
    let app = elm_magic::mount!(Optional);
    app.expect_text("always");
    app.assert_hidden("shown");
}

#[test]
fn if_without_else_renders_when_true() {
    let app = elm_magic::mount_with::<Optional>(OptionalProps {
        visible: Some(true),
        ..Default::default()
    });
    app.expect_text("shown");
    app.expect_text("always");
}

// ── 중첩 ────────────────────────────────────────────────────
elm_magic::view! {
    fn Nested(n = 0) {
        <Col>
            <If when={n > 0}>
                <If when={n > 10}>
                    <Text>"many"</Text>
                <Else>
                    <Text>"few"</Text>
                </Else>
                </If>
            </If>
        </Col>
    }
}

#[test]
fn nested_if_takes_inner_then_branch() {
    let app = elm_magic::mount_with::<Nested>(NestedProps {
        n: Some(20),
        ..Default::default()
    });
    app.expect_text("many");
    app.assert_hidden("few");
}

#[test]
fn nested_if_takes_inner_else_branch() {
    let app = elm_magic::mount_with::<Nested>(NestedProps {
        n: Some(5),
        ..Default::default()
    });
    app.expect_text("few");
    app.assert_hidden("many");
}

#[test]
fn nested_if_outer_false_renders_neither() {
    let app = elm_magic::mount!(Nested);
    app.assert_hidden("many");
    app.assert_hidden("few");
}

// ── 동적 상태 변화 ──────────────────────────────────────────
elm_magic::view! {
    fn Toggle(on = false) {
        <Col>
            <Button on_click={on = !on}>"toggle"</Button>
            <If when={on}>
                <Text>"ON"</Text>
            <Else>
                <Text>"OFF"</Text>
            </Else>
            </If>
        </Col>
    }
}

#[test]
fn if_reacts_to_state_change() {
    let mut app = elm_magic::mount!(Toggle);
    app.expect_text("OFF");
    app.assert_hidden("ON");

    app.click("toggle");
    app.expect_text("ON");
    app.assert_hidden("OFF");

    app.click("toggle");
    app.expect_text("OFF");
}

// ── 본문 전체가 조건부 ──────────────────────────────────────
elm_magic::view! {
    fn Whole(loading = true) {
        <If when={loading}>
            <Spinner />
        <Else>
            <Text>"ready"</Text>
        </Else>
        </If>
    }
}

#[test]
fn body_may_be_a_whole_if() {
    let app = elm_magic::mount!(Whole);
    app.expect_text("[spinner]");

    let app = elm_magic::mount_with::<Whole>(WholeProps {
        loading: Some(false),
        ..Default::default()
    });
    app.expect_text("ready");
    app.assert_hidden("[spinner]");
}

// ── 레이아웃 노드를 만들지 않는다 ───────────────────────────
elm_magic::view! {
    fn Transparent(on = true) {
        <Col>
            <If when={on}>
                <Text>"inner"</Text>
            </If>
        </Col>
    }
}

#[test]
fn if_adds_no_layout_node() {
    let app = elm_magic::mount!(Transparent);
    let root = app.element();
    let children = root.children().expect("Col has children");
    assert_eq!(
        children.len(),
        1,
        "<If> must not add a wrapper node:\n{}",
        app.render_tree()
    );
    assert_eq!(children[0].tag(), "text");
}
