// TDD: 카운터 컴포넌트 — 사양서 3.1 / 테스트 8.1
elm_magic::view! {
    fn Counter(n = 0) {
        <Row>
            <Button on_click={n -= 1}>"-"</Button>
            "Count: {n}"
            <Button on_click={n += 1}>"+"</Button>
        </Row>
    }
}

#[test]
fn counter_initial_render() {
    let app = elm_magic::mount!(Counter);
    app.expect_text("Count: 0");
}

#[test]
fn counter_increments() {
    let mut app = elm_magic::mount!(Counter);
    app.click("+");
    app.expect_text("Count: 1");
    app.click("+");
    app.click("+");
    app.expect_text("Count: 3");
}

#[test]
fn counter_decrements() {
    let mut app = elm_magic::mount!(Counter);
    app.click("-");
    app.expect_text("Count: -1");
}

// 독립 `ui!` 매크로 (사양서 10.2)
#[test]
fn ui_macro_standalone() {
    let el = elm_magic::ui! { <Col class="x"><Text>"hi"</Text><Button disabled={true}>"go"</Button></Col> };
    match &el {
        elm_magic::Element::Col(col) => {
            assert_eq!(col.class, vec!["x".to_string()]);
            assert_eq!(col.children.len(), 2);
        }
        other => panic!("expected Col, got {:?}", std::mem::discriminant(other)),
    }
}

elm_magic::view! {
    fn App() {
        <Col class="app">
            "elm-magic demo"
            <Counter />
        </Col>
    }
}

#[test]
fn nested_component_state_is_isolated() {
    let mut app = elm_magic::mount!(App);
    app.expect_text("elm-magic demo");
    app.expect_text("Count: 0");
    app.click("+");
    app.expect_text("Count: 1");
    // parent text unchanged
    app.expect_text("elm-magic demo");
}

#[test]
fn nested_component_with_props() {
    let mut app = elm_magic::mount_with::<App>(AppProps::default());
    app.click("+");
    app.click("+");
    app.expect_text("Count: 2");
}
#[test]
fn counter_snapshot_tree() {
    let app = elm_magic::mount!(Counter);
    let tree = app.render_tree();
    assert!(tree.contains("Row"), "tree should contain Row:\n{}", tree);
    assert!(tree.contains("Button \"-\""), "tree:\n{}", tree);
    assert!(tree.contains("Button \"+\""), "tree:\n{}", tree);
}
