//! 자원·예산 회귀 — 아레나가 반복 사용에도 **쌓이지 않는지**를 관측 가능한 값으로 고정한다.
//!
//! 벽시계 시간은 flaky하므로 단언하지 않는다. 대신 공개 API로 셀 수 있는 것만 센다:
//! `keyed_slot_count()`, `ctx.arena.stream_count()`, `has_pending_after()`.
//! (성능 자체를 재려면 `benches/` + `#[ignore]` 잡으로 분리한다 — 여기서는 회귀만 본다.)

async fn budget_load(id: i32) -> String {
    format!("v{id}")
}

elm_magic::view! {
    fn BudgetLoader(value = String::new()) {
        <Col>
            <Button on_click={value <- budget_load(1)}>"go"</Button>
            "value: {value}"
        </Col>
    }
}

#[test]
fn repeated_effect_cycles_do_not_grow_arena_state() {
    let mut app = elm_magic::mount!(BudgetLoader);
    let streams = app.ctx.arena.stream_count();
    let slots = app.keyed_slot_count();

    for round in 0..100 {
        app.click("go");
        app.flush();
        assert_eq!(
            app.ctx.arena.stream_count(),
            streams,
            "{round}번째 사이클에서 스트림 수가 늘었다"
        );
        assert_eq!(
            app.keyed_slot_count(),
            slots,
            "{round}번째 사이클에서 keyed 슬롯이 늘었다"
        );
    }
    app.assert_text("value: v1");
}

elm_magic::view! {
    fn BudgetDelayed(loaded = String::new()) {
        <Col>
            <Button on_click={loaded <- budget_load(2) after 100ms}>"slow"</Button>
            "loaded: {loaded}"
        </Col>
    }
}

#[test]
fn delayed_effect_queue_drains_after_the_clock_advances() {
    let mut app = elm_magic::mount!(BudgetDelayed);
    assert!(!app.has_pending_after(), "예약된 지연 효과가 없다");

    app.click("slow");
    assert!(app.has_pending_after(), "클릭이 지연 효과를 예약했다");
    app.advance(99);
    assert!(app.has_pending_after(), "아직 지연이 남았다");
    app.advance(1);
    assert!(!app.has_pending_after(), "실행 후 큐가 비었다");
    app.assert_text("loaded: v2");

    // 반복해도 큐가 쌓이지 않는다
    for round in 0..20 {
        app.click("slow");
        app.advance(100);
        assert!(
            !app.has_pending_after(),
            "{round}번째 반복 후에도 큐가 비어 있다"
        );
    }
}

fn budget_feed() -> Vec<i32> {
    vec![1]
}

elm_magic::view! {
    fn BudgetFeed(total = 0) {
        on_message(budget_feed(), v) { total += v; }
        <Text>"total: {total}"</Text>
    }
}

#[test]
fn subscription_count_reflects_live_streams() {
    // 현재 의미론(0.7.4 실측): `stream_count()`는 **살아 있는 구독 태스크 수**다.
    // 값을 모두 밀어넣고 끝난 스트림은 정리되어 수가 줄어든다 → 반복 사용에서 누적되지 않는다.
    let mut app = elm_magic::mount!(BudgetFeed);
    assert_eq!(app.ctx.arena.stream_count(), 1, "마운트가 구독을 등록했다");

    app.pump();
    app.assert_text("total: 1");
    assert_eq!(
        app.ctx.arena.stream_count(),
        1,
        "첫 pump는 값을 밀어넣는다 (끝난 것은 다음 pump에서 관측된다)"
    );

    app.pump();
    assert_eq!(
        app.ctx.arena.stream_count(),
        0,
        "값을 모두 밀어넣은 구독은 끝나고 정리된다"
    );

    for round in 0..10 {
        app.pump();
        assert_eq!(
            app.ctx.arena.stream_count(),
            0,
            "{round}번째 pump 후에도 0 (구독이 쌓이지 않는다)"
        );
    }
}

elm_magic::view! {
    fn BudgetCounter(n = 0) {
        <Col>
            "n: {n}"
            <Button on_click={n += 1}>"inc"</Button>
        </Col>
    }
}

#[test]
fn many_sequential_instances_are_stable() {
    // 순차 마운트/드롭 50회 — 전역 상태가 새 인스턴스로 새지 않는다 (0.7.4 의미론)
    for _ in 0..50 {
        let mut app = elm_magic::mount!(BudgetCounter);
        app.assert_text("n: 0");
        app.click("inc");
        app.assert_text("n: 1");
    }
}
