// v0.2 — 라이프사이클 (사양서 5.2, 5.3, 3.rs 패턴 1/15)

// ── on_key (사양서 5.3 / 3.rs 15 키보드 단축키) ─────────────
elm_magic::view! {
    fn Editor(text = String::new()) {
        on_key("Ctrl+S") { text = String::from("saved") }
        on_key("Ctrl+Z") { text = String::from("undone") }
        <Text>"{text}"</Text>
    }
}

#[test]
fn on_key_dispatches() {
    let mut app = elm_magic::mount!(Editor);
    app.press_key("Ctrl+S");
    app.expect_text("saved");
    app.press_key("Ctrl+Z");
    app.expect_text("undone");
}

// ── on_tick (사양서 5.2 / 2.rs 9 실시간) ────────────────────
elm_magic::view! {
    fn Clock(now = 0) {
        on_tick(100) { now += 1 }
        <Text>"{now}"</Text>
    }
}


#[test]
fn on_tick_fires_when_advanced() {
    let mut app = elm_magic::mount!(Clock);
    app.expect_text("0");
    app.advance(100);
    app.expect_text("1");
    app.advance(100);
    app.expect_text("2");
    // 짧은 advance는 다음 주기까지 대기
    app.advance(50);
    app.expect_text("2");
    app.advance(50);
    app.expect_text("3");
}

// ── on_change after (사양서 5.2 / 1.rs 10 디바운스 검색) ────
elm_magic::view! {
    fn Search(query = String::new(), results = String::new()) {
        on_change(query) after 300 { results = format!("hits:{}", query) }
        <Col>
            <Input value={query.clone()} on_change={query = _} />
            <Text>"{results}"</Text>
        </Col>
    }
}

#[test]
fn on_change_after_debounces() {
    let mut app = elm_magic::mount!(Search);
    app.type_("rust");
    // 300ms 전: 아직 실행 안 됨
    app.advance(200);
    app.expect_text("");
    app.advance(100);
    app.expect_text("hits:rust");
}

#[test]
fn on_change_after_restarts_timer_on_new_input() {
    let mut app = elm_magic::mount!(Search);
    app.type_("ru");
    app.advance(250);
    app.type_("rust");
    // 이전 타이핑 기준 300ms 지났지만 값이 바뀌어 타이머 리셋
    app.advance(50);
    app.expect_text("");
    app.advance(250);
    app.expect_text("hits:rust");
}
