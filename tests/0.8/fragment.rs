//! 0.8.0 프래그먼트 — `<> … </>` (사양: `0.8-preview.md` §3.1).
//!
//! 계약:
//! - 자식들을 **레이아웃 없이** 묶는다 (Fragment 노드, `tag() == ""`).
//! - 본문 전체/조건/반복 안쪽 어디서나 쓸 수 있다.
//!
//! 이 파일은 `cargo test --test v0_8_fragment`로 돈다.

// ── 본문 전체가 프래그먼트 ──────────────────────────────────
elm_magic::view! {
    fn Body() {
        <>
            <Text>"x"</Text>
            <Text>"y"</Text>
        </>
    }
}

#[test]
fn fragment_body_is_layout_free() {
    let app = elm_magic::mount!(Body);
    assert_eq!(
        app.element().tag(),
        "",
        "fragment must not have a layout tag"
    );
    let children = app.element().children().expect("fragment has children");
    assert_eq!(children.len(), 2);
    app.expect_text("x");
    app.expect_text("y");
}

// ── 레이아웃 자식으로 쓰기 (순서 보존) ──────────────────────
elm_magic::view! {
    fn Grouped() {
        <Col>
            <>
                <Text>"a"</Text>
                <Text>"b"</Text>
            </>
            <Text>"c"</Text>
        </Col>
    }
}

#[test]
fn fragment_renders_children_in_order() {
    let app = elm_magic::mount!(Grouped);
    let all = app.text();
    let a = all.find('a').expect("a");
    let b = all.find('b').expect("b");
    let c = all.find('c').expect("c");
    assert!(a < b && b < c, "order must be preserved: {all:?}");
}

// ── 중첩 프래그먼트 ─────────────────────────────────────────
elm_magic::view! {
    fn Nested() {
        <Col>
            <>
                <Text>"outer"</Text>
                <>
                    <Text>"inner"</Text>
                </>
            </>
        </Col>
    }
}

#[test]
fn nested_fragments_render_all_leaves() {
    let app = elm_magic::mount!(Nested);
    app.expect_text("outer");
    app.expect_text("inner");
}

// ── 조건/반복 안쪽의 프래그먼트 ─────────────────────────────
elm_magic::view! {
    fn InControl(on = true, items: Vec<String> = vec![]) {
        <Col>
            <If when={on}>
                <>
                    <Text>"if-x"</Text>
                    <Text>"if-y"</Text>
                </>
            </If>
            <For each={items} as={i}>
                <>
                    <Text>"{i}-1"</Text>
                    <Text>"{i}-2"</Text>
                </>
            </For>
        </Col>
    }
}

#[test]
fn fragment_inside_if_renders_all_children() {
    let app = elm_magic::mount!(InControl);
    app.expect_text("if-x");
    app.expect_text("if-y");
}

#[test]
fn fragment_inside_if_is_dropped_when_false() {
    let app = elm_magic::mount_with::<InControl>(InControlProps {
        on: Some(false),
        ..Default::default()
    });
    app.assert_hidden("if-x");
    app.assert_hidden("if-y");
}

#[test]
fn fragment_inside_for_repeats_per_item() {
    let app = elm_magic::mount_with::<InControl>(InControlProps {
        items: Some(vec!["a".to_string(), "b".to_string()]),
        ..Default::default()
    });
    app.expect_text("a-1");
    app.expect_text("a-2");
    app.expect_text("b-1");
    app.expect_text("b-2");
}
