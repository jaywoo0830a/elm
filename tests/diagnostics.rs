//! 진단 — 사용자가 실제로 만나는 **panic 메시지**를 계약으로 고정한다.
//!
//! 사양서 13은 "Span 뭉개짐 → `syn::Error::new_spanned` 필수"라고 트레이드오프를
//! 인정한다. 매크로 라이브러리에서 메시지는 곧 API이므로, 메시지에 **원인과 해결책**
//! (어떤 prop인지, 어떤 키인지, 무엇을 대신 쓰면 되는지)이 남는지 검사한다.
//!
//! 기존 `tests/callbacks.rs`가 `is_err()`만 보는 지점을 메시지 수준으로 강화한다.

use std::panic::{catch_unwind, AssertUnwindSafe};

/// `catch_unwind`가 돌려준 payload에서 메시지를 꺼낸다.
fn panic_message(err: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = err.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        String::from("<non-string panic>")
    }
}

// ── 필수 prop 누락 ─────────────────────────────────────────

elm_magic::view! {
    fn DiagNeeds(value: String, count: i32) {
        <Text>"{value}:{count}"</Text>
    }
}

#[test]
fn missing_required_prop_names_the_field_and_the_fix() {
    let err = catch_unwind(|| {
        let _ = elm_magic::mount!(DiagNeeds);
    })
    .unwrap_err();
    let msg = panic_message(err);

    assert!(
        msg.contains("value"),
        "누락된 prop 이름이 메시지에 있어야 한다: {msg}"
    );
    assert!(msg.contains("prop"), "무엇이 문제인지 말해야 한다: {msg}");
    assert!(
        msg.contains("mount_with"),
        "해결책(대안 API)을 알려줘야 한다: {msg}"
    );
}

// ── 등록되지 않은 키 입력 ──────────────────────────────────

elm_magic::view! {
    fn DiagKeys(text = String::new()) {
        on_key("Ctrl+S") { text = String::from("saved") }
        <Text>"{text}"</Text>
    }
}

#[test]
fn unregistered_key_message_names_the_key() {
    let err = catch_unwind(AssertUnwindSafe(|| {
        let mut app = elm_magic::mount!(DiagKeys);
        app.press_key("F5");
    }))
    .unwrap_err();
    let msg = panic_message(err);

    assert!(msg.contains("F5"), "어떤 키였는지 알려줘야 한다: {msg}");
    assert!(
        msg.contains("on_key"),
        "등록 지점(`on_key`)을 알려줘야 한다: {msg}"
    );
}

// ── navigate 타입 불일치 ───────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
#[allow(dead_code)] // `User`는 잘못된 타입의 navigate를 만들기 위한 대조군이다
enum DiagRoute {
    Home,
    User(i32),
}

elm_magic::view! {
    fn DiagRouter(route: DiagRoute = DiagRoute::Home) {
        on_navigate(|r| route = r)
        <Text>"{route:?}"</Text>
    }
}

#[test]
fn navigate_type_mismatch_is_actionable() {
    let err = catch_unwind(AssertUnwindSafe(|| {
        let mut app = elm_magic::mount!(DiagRouter);
        app.navigate_value(42i32);
    }))
    .unwrap_err();
    let msg = panic_message(err);

    assert!(
        msg.contains("navigate"),
        "어떤 기능인지 알려줘야 한다: {msg}"
    );
    assert!(
        msg.contains("타입") || msg.contains("type"),
        "원인이 타입 불일치임을 말해야 한다: {msg}"
    );
}

// ── 효과(effect)의 panic은 삼켜지지 않는다 ─────────────────

async fn diag_boom() -> String {
    panic!("diag effect exploded");
}

elm_magic::view! {
    fn DiagBoom(value = String::new()) {
        <Col>
            <Button on_click={value <- diag_boom()}>"go"</Button>
            "v: {value}"
        </Col>
    }
}

#[test]
fn effect_panic_propagates_at_flush() {
    let err = catch_unwind(|| {
        let mut app = elm_magic::mount!(DiagBoom);
        app.click("go");
        app.flush();
    })
    .unwrap_err();
    let msg = panic_message(err);

    assert!(
        msg.contains("diag effect exploded"),
        "효과의 panic이 flush에서 그대로 드러나야 한다: {msg}"
    );
}

// ── 메시지가 매크로 내부 구현을 새지 않는다 ────────────────

#[test]
fn diagnostic_messages_do_not_leak_macro_internals() {
    let mut messages = Vec::new();

    let missing = catch_unwind(|| {
        let _ = elm_magic::mount!(DiagNeeds);
    })
    .unwrap_err();
    messages.push(panic_message(missing));

    let bad_key = catch_unwind(AssertUnwindSafe(|| {
        let mut app = elm_magic::mount!(DiagKeys);
        app.press_key("F9");
    }))
    .unwrap_err();
    messages.push(panic_message(bad_key));

    let bad_nav = catch_unwind(AssertUnwindSafe(|| {
        let mut app = elm_magic::mount!(DiagRouter);
        app.navigate_value(7i32);
    }))
    .unwrap_err();
    messages.push(panic_message(bad_nav));

    for msg in messages {
        for leak in ["__elm", "proc macro panicked", "Any { .. }"] {
            assert!(
                !msg.contains(leak),
                "전개 내부 표현이 사용자 메시지에 새면 안 된다 ({leak}): {msg}"
            );
        }
    }
}
