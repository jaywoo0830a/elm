//! v0.5 — 구독 계열 (사양서 5.3, 5.4): on_message / on_event / on_net_change /
//! on_navigate + 스트림 목 / 이벤트 버스 / 라우팅 / 네트워크 상태

// ── on_message — 스트림 구독 (사양서 5.4) ───────────────────

fn messages(room: String) -> Vec<String> {
    vec![format!("real-{room}")]
}

elm_magic::view! {
    fn Chat(room = String::from("lobby"), msgs: Vec<String> = vec![]) {
        on_message(messages(room.clone()), m) { msgs.push(m); }
        <Col>
            "room: {room}"
            {msgs.map(|m| <Row>"{m}"</Row>)}
        </Col>
    }
}

#[test]
fn on_message_pumps_stream_values_into_state() {
    let mut app = elm_magic::mount_with::<Chat>(ChatProps {
        room: Some(String::from("dev")),
        ..Default::default()
    });
    app.pump();
    app.assert_text("real-dev");
}

#[test]
fn on_message_stream_is_mockable() {
    let app = elm_magic::mount!(Chat);
    // mount 후에 등록해도 된다 (첫 pump에서 목을 확인)
    let mut app = app;
    elm_magic::mock_stream!(app, messages, ["m1".to_string(), "m2".to_string()]);
    app.pump();
    app.pump();
    app.assert_text("m1");
    app.assert_text("m2");
    app.assert_hidden("real-lobby");
}

// ── on_event + bus.emit — 크로스 컴포넌트 이벤트 버스 (사양서 5.3) ──

elm_magic::view! {
    fn Toolbar() {
        <Row>
            <Button on_click={bus.emit(RefreshRequested)}>"refresh"</Button>
            <Button on_click={bus.emit(SyncNow)}>"sync"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn DataTable(hits = 0, synced = 0) {
        on_event(RefreshRequested) { hits += 1 }
        on_event(SyncNow) { synced += 1 }
        <Col>
            "hits: {hits}"
            "synced: {synced}"
        </Col>
    }
}

elm_magic::view! {
    fn BusRoot() {
        <Col>
            <Toolbar />
            <DataTable />
        </Col>
    }
}

#[test]
fn event_bus_reaches_other_components() {
    let mut app = elm_magic::mount!(BusRoot);
    app.assert_text("hits: 0");
    app.click("refresh");
    app.assert_text("hits: 1");
    app.click("refresh");
    app.assert_text("hits: 2");
    app.click("sync");
    app.assert_text("synced: 1");
    app.assert_text("hits: 2");
}

#[test]
fn emit_from_test_api() {
    let mut app = elm_magic::mount!(BusRoot);
    app.emit("RefreshRequested");
    app.assert_text("hits: 1");
}

// ── on_navigate — 라우팅 (사양서 5.3) ───────────────────────

elm_magic::view! {
    fn Router(path = String::from("/"), visits = 0) {
        on_navigate(|p| path = p); // 값은 이미 소유된 복제본 (nav_take)
        <Col>
            "path: {path}"
            "visits: {visits}"
        </Col>
    }
}

// 타입 있는 라우트: `app.navigate_value(Route::User(42))`
#[derive(Clone, PartialEq, Debug)]
enum Route {
    Home,
    User(i32),
}

elm_magic::view! {
    fn TypedRouter(route: Route = Route::Home) {
        on_navigate(|r| route = r)
        <Col>
            "route: {route:?}"
        </Col>
    }
}

#[test]
fn navigate_value_passes_typed_route() {
    let mut app = elm_magic::mount!(TypedRouter);
    app.assert_text("route: Home");
    app.navigate_value(Route::User(42));
    app.assert_text("route: User(42)");
}

#[test]
fn navigate_delivers_path() {
    let mut app = elm_magic::mount!(Router);
    app.assert_text("path: /");
    app.navigate("/users/42");
    app.assert_text("path: /users/42");
    app.navigate("/post/7/hello");
    app.assert_text("path: /post/7/hello");
}

// ── on_net_change + net.is_online() (사양서 5.3) ────────────

elm_magic::view! {
    fn Offline(online = true, changes = 0) {
        on_net_change {
            online = net.is_online();
            changes += 1;
        }
        <Col>
            "online: {online}"
            "changes: {changes}"
        </Col>
    }
}

#[test]
fn net_change_updates_state() {
    let mut app = elm_magic::mount!(Offline);
    app.assert_text("online: true");
    app.assert_text("changes: 0");
    app.set_online(false);
    app.assert_text("online: false");
    app.assert_text("changes: 1");
    app.set_online(true);
    app.assert_text("online: true");
    app.assert_text("changes: 2");
}

// ── on_unmount은 tests/store.rs에서 검증 (keyed 트리와 함께) ──

#[test]
fn on_message_stream_shows_up_in_stream_count() {
    let app = elm_magic::mount!(Chat);
    assert_eq!(app.ctx.arena.stream_count(), 1);
}
