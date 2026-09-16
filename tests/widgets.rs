// v0.4 — 빌트인 태그 확장 (사양서 1.rs 2/4/5/12/13/14, 3.rs 18)
//
// Spinner, Divider, Strong, Banner, Check, TextArea, Tab, Th, Td, Progress, Modal

elm_magic::view! {
    fn Panel(
        items: Vec<String> = vec![],
        text = String::new(),
        open = false,
        synced = false,
        tab = 0,
    ) {
        <Col>
            <Spinner />
            <Divider />
            <Strong>"Total: 3"</Strong>
            <Banner kind="error">"boom"</Banner>
            <Progress value={0.5} />
            <Check checked={open} on_change={open = !open}>"agree"</Check>
            <Check checked={synced} on_change={synced = _}>"sync"</Check>
            <TextArea value={text.clone()} on_change={text = _} />
            <Row>
                <Tab active={tab == 0} on_click={tab = 0}>"Home"</Tab>
                <Tab active={tab == 1} on_click={tab = 1}>"Stats"</Tab>
            </Row>
            <Row>
                <Th on_click={tab = 1}>"Name"</Th>
                <Td>"cell"</Td>
            </Row>
            {items.map(|i| <Row>"{i}"</Row>)}
            <Modal on_close={open = false}>
                <Text>"modal body"</Text>
                <Button on_click={open = false}>"close"</Button>
            </Modal>
        </Col>
    }
}

fn panel() -> elm_magic::testing::TestApp<Panel> {
    elm_magic::mount!(Panel)
}

#[test]
fn new_tags_render_in_tree() {
    let tree = panel().render_tree();
    for expected in [
        "Spinner",
        "Divider",
        "Strong \"Total: 3\"",
        "Banner kind=\"error\" \"boom\"",
        "Progress 0.5",
        "Check \"agree\" checked=false",
        "TextArea value=\"\"",
        "Tab \"Home\" active",
        "Tab \"Stats\"",
        "Th \"Name\"",
        "Td \"cell\"",
        "Modal",
    ] {
        assert!(tree.contains(expected), "missing {:?} in:\n{}", expected, tree);
    }
}

#[test]
fn new_tags_appear_in_text() {
    let app = panel();
    app.assert_text("[spinner]");
    app.assert_text("[divider]");
    app.assert_text("[progress: 0.5]");
    app.assert_text("Total: 3");
    app.assert_text("boom");
    app.assert_text("[modal]");
    app.assert_text("modal body");
}

#[test]
fn check_toggles_state() {
    let mut app = panel();
    app.toggle("agree");
    assert!(app.render_tree().contains("Check \"agree\" checked=true"));
    app.toggle("agree");
    assert!(app.render_tree().contains("Check \"agree\" checked=false"));
}

#[test]
fn check_value_event_uses_underscore() {
    let mut app = panel();
    app.set_check("sync", true);
    assert!(app.render_tree().contains("Check \"sync\" checked=true"));
}

#[test]
fn textarea_typing() {
    let mut app = panel();
    app.type_into("textarea", "hello");
    assert!(
        app.render_tree().contains("TextArea value=\"hello\""),
        "tree:\n{}",
        app.render_tree()
    );
}

#[test]
fn tab_click_switches_active_tab() {
    let mut app = panel();
    app.click("Stats");
    let tree = app.render_tree();
    assert!(tree.contains("Tab \"Stats\" active"), "tree:\n{}", tree);
    assert!(!tree.contains("Tab \"Home\" active"), "tree:\n{}", tree);
}

#[test]
fn th_click_runs_handler() {
    let mut app = panel();
    app.click("Name");
    assert!(app.render_tree().contains("Tab \"Stats\" active"));
}

#[test]
fn modal_children_are_clickable() {
    let mut app = panel();
    app.click("close");
    assert!(app.render_tree().contains("Check \"agree\" checked=false"));
}

#[test]
fn list_items_render_inside_panel() {
    let app = elm_magic::mount_with::<Panel>(PanelProps {
        items: vec!["alpha".to_string()],
        ..Default::default()
    });
    app.assert_text("alpha");
    app.assert_text("cell");
}