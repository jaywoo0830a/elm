//! v0.5 통합 — 전역 상태(`#[store]`) + keyed 트리 + 구독 + 콜백 prop + children 를
//! 한 컴포넌트 트리에서 함께 검증한다.

use elm_magic::prelude::*;

#[store]
struct App {
    picks: i32,
    theme_dark: bool,
    closed: i32,
}

#[derive(Clone, PartialEq)]
struct Item {
    id: i32,
    name: String,
}

fn feed(room: String) -> Vec<String> {
    vec![format!("live-{room}")]
}

elm_magic::view! {
    fn ItemRow(item: Item, on_pick: fn(i32)) {
        <Row on_click={on_pick(item.id)}>
            "{item.name}"
        </Row>
    }
}

elm_magic::view! {
    fn Panel() {
        <Col>
            "panel picks: {app.picks}"
            "panel closed: {app.closed}"
        </Col>
    }
}

elm_magic::view! {
    fn Session() {
        on_unmount { app.closed += 1 }
        <Text>"session"</Text>
    }
}

elm_magic::view! {
    fn Shell(
        items: Vec<Item> = vec![],
        picked = 0,
        msgs: Vec<String> = vec![],
        room = String::from("dev"),
        show = true,
    ) {
        on_message(feed(room.clone()), m) { msgs.push(m); }
        on_event(Refresh) { app.picks += 1 }
        <Col>
            <Card title="shell">
                <Col>{items.map(|i| <ItemRow key={i.id} item={i} on_pick={picked = _} />)}</Col>
                <Button on_click={app.theme_dark = !app.theme_dark}>"theme"</Button>
                <Button on_click={bus.emit(Refresh)}>"refresh"</Button>
                {if show { <Session /> } else { <Text>"hidden"</Text> }}
                <Button on_click={show = !show}>"toggle"</Button>
            </Card>
            "picked: {picked}"
            "dark: {app.theme_dark}"
            {msgs.map(|m| <Row>"{m}"</Row>)}
            <Panel />
        </Col>
    }
}

elm_magic::view! {
    fn Card(title = String::new()) {
        <Col>"card: {title}"{children}</Col>
    }
}

fn shell() -> elm_magic::testing::TestApp<Shell> {
    elm_magic::mount_with::<Shell>(ShellProps {
        items: Some(vec![
            Item { id: 1, name: "one".to_string() },
            Item { id: 2, name: "two".to_string() },
        ]),
        ..Default::default()
    })
}

#[test]
fn callback_prop_updates_parent_state() {
    let mut app = shell();
    app.assert_text("picked: 0");
    app.click("two");
    app.assert_text("picked: 2");
    app.click("one");
    app.assert_text("picked: 1");
}

#[test]
fn store_is_shared_across_children() {
    let mut app = shell();
    app.assert_text("panel closed: 0");
    app.click("theme");
    app.assert_text("dark: true");
}

#[test]
fn component_children_render_inside_card() {
    let app = shell();
    app.assert_text("card: shell");
    app.assert_text("panel picks: 0");
}

#[test]
fn stream_mock_feeds_messages() {
    let app = shell();
    let mut app = app;
    elm_magic::mock_stream!(app, feed, ["hello".to_string()]);
    app.pump();
    app.assert_text("hello");
    app.assert_hidden("live-dev");
}

#[test]
fn event_bus_reaches_store_and_unmount_runs() {
    let mut app = shell();
    app.click("refresh");
    // 이벤트가 **전역 상태**를 바꿨다 (Shell의 지역 상태 picked는 그대로)
    app.assert_text("panel picks: 1");
    app.assert_text("picked: 0");
    // Session unmount → on_unmount가 전역 상태를 갱신
    app.click("toggle");
    app.assert_text("hidden");
    app.assert_text("panel closed: 1");
}

#[test]
fn keyed_children_still_dispatch_callbacks() {
    // keyed 자식의 상태 보존 상세는 tests/store.rs — 여기서는 key + 콜백 prop이
    // 함께 쓰여도 클릭이 올바른 값으로 전달되는지 확인한다.
    let mut app = shell();
    app.click("one");
    app.assert_text("picked: 1");
    app.click("two");
    app.assert_text("picked: 2");
    app.click("one");
    app.assert_text("picked: 1");
}


