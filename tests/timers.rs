// v0.4 — 시간 리터럴과 지연 효과 (사양서 5.1, 5.2, 8.2)

async fn api_user(id: i32) -> String {
    format!("user-{id}")
}

// ── 1. `on_tick(500ms)` — 시간 리터럴 ──────────────────────
elm_magic::view! {
    fn Clock(now = 0) {
        on_tick(500ms) { now += 1 }
        <Text>"{now}"</Text>
    }
}

#[test]
fn on_tick_accepts_duration_literal() {
    let mut app = elm_magic::mount!(Clock);
    app.assert_text("0");
    app.advance(500);
    app.assert_text("1");
    app.advance(500);
    app.assert_text("2");
    app.advance(500);
    app.assert_text("3");
    // 주기가 덜 지나면 실행되지 않는다
    app.advance(200);
    app.assert_text("3");
}

// ── 2. `on_change(x) after 300ms` — 리터럴 + 디바운스 ───────
elm_magic::view! {
    fn Search(query = String::new(), hits = String::new()) {
        on_change(query) after 300ms { hits = format!("hits:{}", query) }
        <Col>
            <Input value={query.clone()} on_change={query = _} />
            "{hits}"
        </Col>
    }
}

#[test]
fn on_change_after_accepts_duration_literal() {
    let mut app = elm_magic::mount!(Search);
    app.type_into("input", "rust");
    app.advance(299);
    app.assert_hidden("hits:rust");
    app.advance(1);
    app.assert_text("hits:rust");
}

// ── 3. `after` 없는 `on_change` — 즉시 감시 ────────────────
elm_magic::view! {
    fn Echo(query = String::new(), mirrored = String::new()) {
        on_change(query) { mirrored = query.clone() }
        <Col>
            <Input value={query.clone()} on_change={query = _} />
            "mirror: {mirrored}"
        </Col>
    }
}

#[test]
fn on_change_without_after_fires_on_change() {
    let mut app = elm_magic::mount!(Echo);
    app.type_into("input", "rust");
    app.assert_text("mirror: rust");
    // 값이 그대로면 다시 실행되지 않는다
    app.advance(100);
    app.assert_text("mirror: rust");
}

// ── 4. `<- f() after 300ms` — 지연 효과 ────────────────────
elm_magic::view! {
    fn Profile(id = 1, user = String::new(), fresh = String::new()) {
        on_mount {
            if user.is_empty() { user <- api_user(id) }
            fresh <- api_user(id) after 300ms
        }
        <Col>
            "user: {user}"
            "fresh: {fresh}"
        </Col>
    }
}

#[test]
fn delayed_effect_waits_for_the_clock() {
    let mut app = elm_magic::mount!(Profile);
    app.flush(); // 즉시 효과만 실행된다
    app.assert_text("user: user-1");
    app.assert_text("fresh: ");
    assert!(app.has_pending_after(), "300ms 효과가 예약되어 있어야 한다");

    app.advance(299);
    app.assert_text("fresh: ");
    app.advance(1);
    app.assert_text("fresh: user-1");
    assert!(!app.has_pending_after());
}

// ── 5. `after 1min` — 분 단위 리터럴 ───────────────────────
elm_magic::view! {
    fn Slow(id = 1, loaded = String::new()) {
        on_mount { loaded <- api_user(id) after 1min }
        <Text>"{loaded}"</Text>
    }
}

#[test]
fn delayed_effect_minutes_literal() {
    let mut app = elm_magic::mount!(Slow);
    app.flush();
    app.assert_hidden("user-1");
    app.advance(59_999);
    app.assert_hidden("user-1");
    app.advance(1);
    app.assert_text("user-1");
}

// ── 6. 클릭에서 시작한 지연 효과 + mock 조합 (사양서 8.2) ───
elm_magic::view! {
    fn Refresh(id = 1, data = String::new()) {
        <Col>
            <Button on_click={data <- api_user(id) after 100ms}>"refresh"</Button>
            "data: {data}"
        </Col>
    }
}

#[test]
fn delayed_effect_from_click_is_mockable() {
    let mut app = elm_magic::mount!(Refresh);
    let mut app = app.mock(api_user, |id: i32| format!("mocked-{id}"));
    app.click("refresh");
    app.advance(99);
    app.assert_text("data: ");
    app.advance(1);
    app.assert_text("data: mocked-1");
}
