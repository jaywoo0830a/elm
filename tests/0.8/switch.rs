//! 0.8.0 다분기 태그 — `<Switch>` / `<Case>` / `<Default>` (사양: `0.8-preview.md` §3.1).
//!
//! 계약:
//! - `on={expr}`을 각 `<Case when={PATTERN}>`과 **패턴 매칭**한다
//!   (`Status::Failed(e)`처럼 바인딩 허용).
//! - 일치하는 `<Case>`가 없으면 `<Default>` 자식, `<Default>`도 없으면 아무것도 안 그린다.
//! - `<Switch>`는 레이아웃 노드를 만들지 않는다 (선택된 분기 요소가 그대로 자식).
//!
//! 이 파일은 `cargo test --test v0_8_switch`로 돈다.

#[derive(Clone, PartialEq, Debug)]
enum Filter {
    All,
    Active,
    Done,
}

#[derive(Clone, PartialEq, Debug)]
enum Status {
    Idle,
    Loading,
    Failed(String),
}

// ── 기본 3분기 + Default ────────────────────────────────────
elm_magic::view! {
    fn Filters(filter: Filter = Filter::All) {
        <Col>
            <Switch on={filter}>
                <Case when={Filter::All}>
                    <Text>"all"</Text>
                </Case>
                <Case when={Filter::Active}>
                    <Text>"active"</Text>
                </Case>
                <Default>
                    <Text>"done"</Text>
                </Default>
            </Switch>
        </Col>
    }
}

#[test]
fn switch_takes_matching_case() {
    let app = elm_magic::mount_with::<Filters>(FiltersProps {
        filter: Some(Filter::Active),
        ..Default::default()
    });
    app.expect_text("active");
    app.assert_hidden("all");
    app.assert_hidden("done");
}

#[test]
fn switch_falls_back_to_default() {
    let app = elm_magic::mount_with::<Filters>(FiltersProps {
        filter: Some(Filter::Done),
        ..Default::default()
    });
    app.expect_text("done");
    app.assert_hidden("all");
    app.assert_hidden("active");
}

#[test]
fn switch_default_variant_matches_first_case() {
    let app = elm_magic::mount!(Filters);
    app.expect_text("all");
}

#[test]
fn switch_is_transparent_no_wrapper_node() {
    let app = elm_magic::mount!(Filters);
    let children = app.element().children().expect("Col has children");
    assert_eq!(
        children.len(),
        1,
        "<Switch> must not add a wrapper node:\n{}",
        app.render_tree()
    );
}

// ── 패턴 바인딩 + Default 없음 ──────────────────────────────
elm_magic::view! {
    fn StatusView(status: Status = Status::Idle) {
        <Col>
            <Switch on={status}>
                <Case when={Status::Idle}>
                    <Text>"idle"</Text>
                </Case>
                <Case when={Status::Failed(e)}>
                    <Banner kind="error">{e}</Banner>
                </Case>
            </Switch>
            <Text>"end"</Text>
        </Col>
    }
}

#[test]
fn switch_binds_pattern_value() {
    let app = elm_magic::mount_with::<StatusView>(StatusViewProps {
        status: Some(Status::Failed("boom".to_string())),
        ..Default::default()
    });
    app.expect_text("boom");
    app.expect_text("end");
    app.assert_hidden("idle");
}

#[test]
fn switch_without_default_renders_nothing_on_no_match() {
    let app = elm_magic::mount_with::<StatusView>(StatusViewProps {
        status: Some(Status::Loading),
        ..Default::default()
    });
    app.expect_text("end");
    app.assert_hidden("idle");
    app.assert_hidden("boom");
}

// ── 리터럴 패턴 ─────────────────────────────────────────────
elm_magic::view! {
    fn Number(n = 0) {
        <Col>
            <Switch on={n}>
                <Case when={0}>
                    <Text>"zero"</Text>
                </Case>
                <Case when={1}>
                    <Text>"one"</Text>
                </Case>
                <Default>
                    <Text>"many"</Text>
                </Default>
            </Switch>
        </Col>
    }
}

#[test]
fn switch_matches_literal_patterns() {
    let zero = elm_magic::mount!(Number);
    zero.expect_text("zero");

    let one = elm_magic::mount_with::<Number>(NumberProps {
        n: Some(1),
        ..Default::default()
    });
    one.expect_text("one");

    let many = elm_magic::mount_with::<Number>(NumberProps {
        n: Some(7),
        ..Default::default()
    });
    many.expect_text("many");
}

// ── 동적 상태 변화 ──────────────────────────────────────────
elm_magic::view! {
    fn Tabs(filter: Filter = Filter::All) {
        <Col>
            <Button on_click={filter = Filter::Active}>"to-active"</Button>
            <Switch on={filter}>
                <Case when={Filter::All}>
                    <Text>"tab-all"</Text>
                </Case>
                <Default>
                    <Text>"tab-other"</Text>
                </Default>
            </Switch>
        </Col>
    }
}

#[test]
fn switch_reacts_to_state_change() {
    let mut app = elm_magic::mount!(Tabs);
    app.expect_text("tab-all");

    app.click("to-active");
    app.expect_text("tab-other");
    app.assert_hidden("tab-all");
}
