//! 코어 콜백 계약 회귀 테스트 — **플랫폼 어댑터를 거치지 않는다**.
//!
//! 어댑터(egui/gpui)는 이 계약 위에 얹히므로, 여기서 깨지면 모든 어댑터가
//! 함께 깨진다. 그래서 "화면에 그려졌는가"가 아니라 **아레나가 기대한 순서·
//! 횟수·값으로 바뀌었는가**만 본다.
//!
//! 고정하는 계약:
//! - `on_click` — 클릭마다 정확히 한 번
//! - `on_change`(값/bool) / `on_enter` — 새 값을 그대로 받는다
//! - `on_change … after` — **마운트 시 발화하지 않는다**, 디바운스는 마지막
//!   변경 기준으로 재시작하고 한 번만 발화한다
//! - `on_mount` / `on_unmount` — 인스턴스 수명에 정확히 한 번
//! - `on_key` / `on_tick` — 입력·시계에만 반응한다
//! - `on_event`(bus) / `on_net_change` / `on_navigate` — 실제 변화에만 반응
//! - `<-` 효과 — `flush()`/`advance()` 전에는 실행되지 않는다
//! - 콜백 prop(`fn(T)`) — 값 전달, 미지정이면 조용한 no-op
//!
//! 명세가 모호한 지점(같은 키 중복 등록, 놓친 틱)은 "현재 의미론" 주석과
//! 함께 고정해, 나중에 바뀌면 테스트가 알려주게 한다.

use elm_magic::prelude::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[store]
struct App {
    closed: i32,
    mounts: i32,
}

/// `catch_unwind`가 돌려준 panic payload에서 메시지를 꺼낸다.
fn panic_message(err: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = err.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        String::from("<non-string panic>")
    }
}

// ── on_click ────────────────────────────────────────────────

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
fn click_fires_the_handler_exactly_once_per_click() {
    let mut app = elm_magic::mount!(Counter);
    app.assert_text("Count: 0");
    app.click("+");
    app.assert_text("Count: 1");
    app.click("+");
    app.assert_text("Count: 2");
    app.click("-");
    app.assert_text("Count: 1");
}

#[test]
fn click_handler_always_sees_the_state_of_the_latest_render() {
    // 핸들러는 마지막 렌더의 트리에서 꺼내므로, 값이 누적돼도 어긋나지 않는다.
    let mut app = elm_magic::mount!(Counter);
    for expected in 1..=5 {
        app.click("+");
        app.assert_text(&format!("Count: {expected}"));
    }
}

#[test]
fn click_on_a_disabled_button_still_dispatches_in_the_headless_harness() {
    // 현재 의미론: `disabled`는 스타일/접근성 질의일 뿐이고, 헤드리스 `click`은
    // 핸들러를 그대로 부른다. 어댑터가 클릭을 막는 책임을 진다.
    let mut app = elm_magic::mount!(Counter);
    let tree = app.render_tree();
    assert!(tree.contains("Button \"+\""), "트리: {tree}");
    app.click("+");
    app.assert_text("Count: 1");
}

// ── on_change(값) / on_enter ────────────────────────────────

elm_magic::view! {
    fn Echo(value = String::new(), events = 0) {
        on_change(value) { events += 1 }
        <Col>
            <Input value={value.clone()} on_change={value = _} />
            "value: {value}"
            "events: {events}"
        </Col>
    }
}

#[test]
fn immediate_on_change_does_not_fire_on_mount() {
    // 마운트는 "변경"이 아니다 — 초기값을 기준선으로 삼아야 한다.
    let app = elm_magic::mount!(Echo);
    app.assert_text("value: ");
    app.assert_text("events: 0");
}

#[test]
fn immediate_on_change_fires_once_per_distinct_value() {
    let mut app = elm_magic::mount!(Echo);
    app.type_("a");
    app.assert_text("value: a");
    app.assert_text("events: 1");
    // 같은 값을 다시 넣으면 변경이 아니다
    app.type_("a");
    app.assert_text("events: 1");
    app.type_("b");
    app.assert_text("events: 2");
    app.type_("");
    app.assert_text("events: 3");
}

#[test]
fn immediate_on_change_does_not_refire_without_a_new_value() {
    let mut app = elm_magic::mount!(Echo);
    app.type_("rust");
    app.assert_text("events: 1");
    // 클럭이 흘러도 값이 그대로면 발화하지 않는다
    app.advance(10_000);
    app.assert_text("events: 1");
}

elm_magic::view! {
    fn Form(name = String::new(), sent = String::new()) {
        <Col>
            <Input value={name.clone()} on_change={name = _} on_enter={sent = _} />
            "name: {name}"
            "sent: {sent}"
        </Col>
    }
}

#[test]
fn on_enter_receives_the_current_value() {
    let mut app = elm_magic::mount!(Form);
    app.assert_text("sent: ");
    app.type_("rust");
    app.assert_text("sent: ");
    app.press_enter();
    app.assert_text("sent: rust");
    // 이름은 그대로 남는다 (on_enter는 값을 바꾸지 않는다)
    app.assert_text("name: rust");
}

// ── on_change(bool) — Check ────────────────────────────────

elm_magic::view! {
    fn Checks(done = false, changes = 0) {
        <Col>
            <Check checked={done} on_change={done = _; changes += 1}>"done"</Check>
            "done: {done}"
            "changes: {changes}"
        </Col>
    }
}

#[test]
fn check_on_change_delivers_the_toggled_value() {
    let mut app = elm_magic::mount!(Checks);
    app.assert_text("done: false");
    app.assert_text("changes: 0");
    app.toggle("done");
    app.assert_text("done: true");
    app.assert_text("changes: 1");
    app.toggle("done");
    app.assert_text("done: false");
    app.assert_text("changes: 2");
}

#[test]
fn set_check_with_the_same_value_is_not_a_change() {
    let mut app = elm_magic::mount!(Checks);
    app.set_check("done", false);
    app.assert_text("changes: 0");
    app.set_check("done", true);
    app.assert_text("changes: 1");
    app.set_check("done", true);
    app.assert_text("changes: 1");
}

// ── on_change(값) after — 디바운스 ──────────────────────────

elm_magic::view! {
    fn Debounced(query = String::new(), hits = 0) {
        on_change(query) after 300 { hits += 1 }
        <Col>
            <Input value={query.clone()} on_change={query = _} />
            "hits: {hits}"
        </Col>
    }
}

#[test]
fn debounce_does_not_fire_on_mount_even_after_the_delay_elapses() {
    // 값이 한 번도 바뀌지 않았는데 시계만 흐르면 발화하면 안 된다.
    let mut app = elm_magic::mount!(Debounced);
    app.assert_text("hits: 0");
    app.advance(1_000);
    app.assert_text("hits: 0");
    app.advance(1_000);
    app.assert_text("hits: 0");
}

#[test]
fn debounce_waits_for_the_delay_then_fires_exactly_once() {
    let mut app = elm_magic::mount!(Debounced);
    app.type_("rust");
    app.advance(299);
    app.assert_text("hits: 0");
    app.advance(1);
    app.assert_text("hits: 1");
    // 발화 후 값이 그대로면 다시 발화하지 않는다
    app.advance(10_000);
    app.assert_text("hits: 1");
}

#[test]
fn debounce_restarts_on_every_change_and_delivers_the_latest_value() {
    let mut app = elm_magic::mount!(Debounced);
    app.type_("ru");
    app.advance(250);
    app.type_("rust");
    app.advance(299);
    app.assert_text("hits: 0");
    app.advance(1);
    app.assert_text("hits: 1");
    // 새 값이 오면 다시 무장된다
    app.type_("el");
    app.advance(300);
    app.assert_text("hits: 2");
}

// ── on_mount / on_unmount ──────────────────────────────────

elm_magic::view! {
    fn Mounted(mounts = 0, n = 0) {
        on_mount { mounts += 1 }
        <Col>
            "mounts: {mounts}"
            <Button on_click={n += 1}>"n"</Button>
        </Col>
    }
}

#[test]
fn on_mount_runs_once_across_many_rerenders() {
    let mut app = elm_magic::mount!(Mounted);
    app.assert_text("mounts: 1");
    app.click("n");
    app.click("n");
    app.advance(500);
    app.assert_text("mounts: 1");
}

elm_magic::view! {
    fn MountCounter() {
        on_mount { app.mounts += 1 }
        <Text>"mounted"</Text>
    }
}

elm_magic::view! {
    fn MountHost(show = true) {
        <Col>
            // `on_mount`는 **렌더 중에** 실행된다 — 그 결과를 읽는 노드는
            // 자식보다 뒤에 와야 같은 프레임에서 반영된다.
            {if show { <MountCounter /> } else { <Text>"hidden"</Text> }}
            "mounts: {app.mounts}"
            <Button on_click={show = !show}>"toggle"</Button>
        </Col>
    }
}

#[test]
fn on_mount_runs_again_after_unmount_and_remount() {
    let mut app = elm_magic::mount!(MountHost);
    app.assert_text("mounts: 1");
    app.click("toggle"); // unmount
    app.assert_text("mounts: 1");
    app.click("toggle"); // remount → 상태 초기화 + on_mount 재실행
    app.assert_text("mounts: 2");
}

elm_magic::view! {
    fn Session() {
        on_unmount { app.closed += 1 }
        <Text>"session"</Text>
    }
}

elm_magic::view! {
    fn SessionHost(show = true) {
        <Col>
            "closed: {app.closed}"
            {if show { <Session /> } else { <Text>"hidden"</Text> }}
            <Button on_click={show = !show}>"toggle"</Button>
        </Col>
    }
}

#[test]
fn on_unmount_runs_exactly_once_when_the_instance_disappears() {
    let mut app = elm_magic::mount!(SessionHost);
    app.assert_text("closed: 0");
    app.click("toggle");
    app.assert_text("closed: 1");
    // 다시 렌더해도 이미 사라진 인스턴스의 핸들러가 또 돌면 안 된다
    app.advance(500);
    app.assert_text("closed: 1");
    app.click("toggle"); // remount
    app.assert_text("closed: 1");
    app.click("toggle"); // 두 번째 unmount
    app.assert_text("closed: 2");
}

// ── on_key ─────────────────────────────────────────────────

elm_magic::view! {
    fn Editor(text = String::new(), presses = 0) {
        on_key("Ctrl+S") { text = String::from("saved"); presses += 1 }
        <Col>
            "text: {text}"
            "presses: {presses}"
        </Col>
    }
}

#[test]
fn on_key_fires_on_every_press() {
    let mut app = elm_magic::mount!(Editor);
    app.press_key("Ctrl+S");
    app.assert_text("text: saved");
    app.assert_text("presses: 1");
    app.press_key("Ctrl+S");
    app.assert_text("presses: 2");
}

#[test]
fn pressing_an_unregistered_key_panics_with_a_clear_message() {
    let mut app = elm_magic::mount!(Editor);
    let err = catch_unwind(AssertUnwindSafe(|| app.press_key("F5"))).unwrap_err();
    let msg = panic_message(err);
    assert!(
        msg.contains("no on_key handler"),
        "등록되지 않은 키는 명확한 메시지로 panic해야 한다: {msg}"
    );
}

elm_magic::view! {
    fn DuplicateKey(text = String::new()) {
        on_key("Enter") { text = String::from("first") }
        on_key("Enter") { text = String::from("second") }
        "text: {text}"
    }
}

#[test]
fn duplicate_key_registration_dispatches_the_first_handler() {
    // 현재 의미론: 같은 키를 두 번 등록하면 첫 핸들러만 디스패치된다.
    let mut app = elm_magic::mount!(DuplicateKey);
    app.press_key("Enter");
    app.assert_text("text: first");
}

// ── on_tick ────────────────────────────────────────────────

elm_magic::view! {
    fn Ticker(ticks = 0, pings = 0) {
        on_tick(100) { ticks += 1 }
        <Col>
            "ticks: {ticks}"
            <Button on_click={pings += 1}>"ping"</Button>
        </Col>
    }
}

#[test]
fn on_tick_ignores_rerenders_without_a_clock_advance() {
    let mut app = elm_magic::mount!(Ticker);
    app.assert_text("ticks: 0");
    for _ in 0..5 {
        app.click("ping");
    }
    app.assert_text("ticks: 0");
    app.advance(99);
    app.assert_text("ticks: 0");
    app.advance(1);
    app.assert_text("ticks: 1");
}

#[test]
fn on_tick_fires_at_each_interval() {
    let mut app = elm_magic::mount!(Ticker);
    app.advance(100);
    app.assert_text("ticks: 1");
    app.advance(100);
    app.assert_text("ticks: 2");
    // 주기보다 짧은 advance는 다음 주기까지 대기
    app.advance(50);
    app.assert_text("ticks: 2");
    app.advance(50);
    app.assert_text("ticks: 3");
}

#[test]
fn on_tick_does_not_catch_up_missed_intervals() {
    // 현재 의미론: 긴 advance는 한 번만 발화한다 (놓친 틱을 몰아서 실행하지 않음).
    let mut app = elm_magic::mount!(Ticker);
    app.advance(350);
    app.assert_text("ticks: 1");
}

// ── 숨은 슬롯이 사용자 상태 슬롯을 밀어내지 않는다 ──────────

elm_magic::view! {
    fn Slots(a = 1, b = 2, c = 3, fires = 0) {
        on_change(b) { fires += 1 }
        on_tick(100) { fires += 0 }
        <Col>
            "a: {a} b: {b} c: {c} fires: {fires}"
            <Button on_click={a += 1}>"a+1"</Button>
        </Col>
    }
}

#[test]
fn lifecycle_hidden_slots_do_not_collide_with_user_state_slots() {
    let mut app = elm_magic::mount!(Slots);
    app.assert_text("a: 1 b: 2 c: 3 fires: 0");
    app.click("a+1");
    app.assert_text("a: 2 b: 2 c: 3 fires: 0");
    app.advance(100);
    app.assert_text("a: 2 b: 2 c: 3 fires: 0");
}

// ── on_event + bus.emit — 크로스 컴포넌트 이벤트 ─────────────

elm_magic::view! {
    fn Receiver(hits = 0, pongs = 0) {
        on_event(Ping) { hits += 1 }
        on_event(Pong) { pongs += 1 }
        <Col>
            "hits: {hits}"
            "pongs: {pongs}"
        </Col>
    }
}

elm_magic::view! {
    fn Sender() {
        <Button on_click={bus.emit(Ping)}>"emit"</Button>
    }
}

elm_magic::view! {
    fn EventRoot() {
        <Col>
            <Sender />
            <Receiver />
        </Col>
    }
}

#[test]
fn on_event_reaches_another_component_once_per_emit() {
    let mut app = elm_magic::mount!(EventRoot);
    app.assert_text("hits: 0");
    app.click("emit");
    app.assert_text("hits: 1");
    app.click("emit");
    app.assert_text("hits: 2");
    // 다른 이벤트는 다른 핸들러만 깨운다
    app.assert_text("pongs: 0");
    app.emit("Pong");
    app.assert_text("pongs: 1");
    app.assert_text("hits: 2");
}

#[test]
fn emitting_an_event_without_a_listener_is_a_noop() {
    // 리스너 없는 이벤트는 panic 없이 조용히 사라져야 한다.
    let mut app = elm_magic::mount!(Sender);
    app.click("emit");
    app.assert_text("emit");
    app.emit("NobodyListens");
    app.assert_text("emit");
}

// ── on_net_change + net.is_online() ────────────────────────

elm_magic::view! {
    fn Net(online = true, changes = 0) {
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
fn net_change_fires_only_on_an_actual_transition() {
    let mut app = elm_magic::mount!(Net);
    app.assert_text("changes: 0");
    app.set_online(false);
    app.assert_text("online: false");
    app.assert_text("changes: 1");
    app.set_online(false); // 같은 값 → 전이가 아니다
    app.assert_text("changes: 1");
    app.set_online(true);
    app.assert_text("online: true");
    app.assert_text("changes: 2");
}

// ── on_navigate ────────────────────────────────────────────

elm_magic::view! {
    fn Router(path = String::from("/")) {
        on_navigate(|p| path = p)
        "path: {path}"
    }
}

#[test]
fn navigate_delivers_the_path() {
    let mut app = elm_magic::mount!(Router);
    app.assert_text("path: /");
    app.navigate("/users/42");
    app.assert_text("path: /users/42");
}

#[test]
fn navigate_with_the_wrong_value_type_panics_instead_of_silently_dropping() {
    let mut app = elm_magic::mount!(Router);
    let err = catch_unwind(AssertUnwindSafe(|| app.navigate_value(42i32))).unwrap_err();
    let msg = panic_message(err);
    assert!(msg.contains("navigate"), "타입 불일치 메시지: {msg}");
}

// ── `<-` 효과 — flush 전에는 실행되지 않는다 ────────────────

async fn load() -> String {
    String::from("loaded")
}

elm_magic::view! {
    fn Effects(x = String::new()) {
        <Col>
            "x: {x}"
            <Button on_click={x <- load()}>"load"</Button>
        </Col>
    }
}

#[test]
fn effect_does_not_run_until_flush() {
    let mut app = elm_magic::mount!(Effects);
    app.click("load");
    app.assert_hidden("loaded");
    assert!(
        app.ctx.arena.pending_count() > 0,
        "효과가 큐에 남아 있어야 한다"
    );
    app.flush();
    app.assert_text("x: loaded");
}

elm_magic::view! {
    fn Delayed(x = String::new()) {
        <Col>
            "x: {x}"
            <Button on_click={x <- load() after 200}>"slow"</Button>
        </Col>
    }
}

#[test]
fn delayed_effect_waits_for_the_clock_then_runs() {
    let mut app = elm_magic::mount!(Delayed);
    app.click("slow");
    app.flush(); // 아직 due가 아니다
    app.assert_hidden("loaded");
    assert!(app.has_pending_after());
    app.advance(199);
    app.assert_hidden("loaded");
    app.advance(1);
    app.assert_text("x: loaded");
    assert!(!app.has_pending_after());
}

// ── 콜백 prop (`fn(T)`) ────────────────────────────────────

elm_magic::view! {
    fn PickRow(item: i32, on_pick: fn(i32)) {
        <Row on_click={on_pick(item)}>"row {item}"</Row>
    }
}

elm_magic::view! {
    fn Picker(items: Vec<i32> = vec![], picked = 0) {
        <Col>
            {items.map(|i| <PickRow item={i} on_pick={picked = _} />)}
            "picked: {picked}"
        </Col>
    }
}

#[test]
fn callback_prop_delivers_each_child_value_to_the_parent() {
    let mut app = elm_magic::mount_with::<Picker>(PickerProps {
        items: Some(vec![1, 2, 3]),
        ..Default::default()
    });
    app.assert_text("picked: 0");
    app.click("row 2");
    app.assert_text("picked: 2");
    app.click("row 3");
    app.assert_text("picked: 3");
}

elm_magic::view! {
    fn NoHandler() {
        <Col>
            <PickRow item={7} />
            "ok"
        </Col>
    }
}

#[test]
fn a_missing_callback_prop_is_a_silent_noop() {
    // 콜백 prop을 넘기지 않으면 `Callback::default()`(no-op)가 쓰인다.
    let mut app = elm_magic::mount!(NoHandler);
    app.assert_text("row 7");
    app.click("row 7");
    app.assert_text("row 7");
    app.assert_text("ok");
}
