//! 0.8.x `IntoView` — 중괄호와 `.map()` 제거 (사양: `0.8-preview.md` §3.2).
//!
//! children 자리의 중괄호 식은 **`IntoView`를 구현한 값이면 무엇이든** 그린다.
//!
//! - `&str` / `String` → 텍스트
//! - 숫자 / `bool` → 자동 표시
//! - `Element` / `Vec<Element>` → 그대로
//! - `Option<T>` → `None`이면 아무것도 안 그림
//! - `Iterator<Item = T>` → 펼쳐서 그림
//!
//! 기존 `{if ..}` / `{items.map(..)}`도 그대로 동작한다.
//! 이 파일은 `cargo test --test v0_8_into_view`로 돈다.

// ── 스칼라 상태값을 중괄호에 그대로 ─────────────────────────
elm_magic::view! {
    fn Values(
        title: String = String::from("hello"),
        count = 42,
        ratio = 1.5,
        flag = true,
        maybe: Option<String> = None,
        items: Vec<String> = vec![],
    ) {
        <Col>
            {title}
            {count}
            {ratio}
            {flag}
            {maybe}
            {items}
        </Col>
    }
}

#[test]
fn state_values_render_directly() {
    let app = elm_magic::mount!(Values);
    app.expect_text("hello");
    app.expect_text("42");
    app.expect_text("1.5");
    app.expect_text("true");
}

#[test]
fn boolean_false_renders() {
    let app = elm_magic::mount_with::<Values>(ValuesProps {
        flag: Some(false),
        ..Default::default()
    });
    app.expect_text("false");
}

#[test]
fn option_some_renders_and_none_is_empty() {
    let app = elm_magic::mount_with::<Values>(ValuesProps {
        maybe: Some(Some("guest".to_string())),
        ..Default::default()
    });
    app.expect_text("guest");

    let app = elm_magic::mount!(Values);
    app.assert_hidden("guest");
}

#[test]
fn vec_renders_each_item() {
    let app = elm_magic::mount_with::<Values>(ValuesProps {
        items: Some(vec!["a".to_string(), "b".to_string()]),
        ..Default::default()
    });
    app.expect_text("a");
    app.expect_text("b");
}

// ── `&str` (borrowed) ───────────────────────────────────────
elm_magic::view! {
    fn Borrowed(label: &'static str = "borrowed") {
        <Col>{label}</Col>
    }
}

#[test]
fn borrowed_str_renders() {
    let app = elm_magic::mount!(Borrowed);
    app.expect_text("borrowed");
}

// ── Element 목록 + 이터레이터 ───────────────────────────────
elm_magic::view! {
    fn Elements(items: Vec<elm_magic::Element> = vec![], labels: Vec<String> = vec![]) {
        <Col>
            {items}
            {labels.iter().map(|l| <Text>"{l}"</Text>)}
        </Col>
    }
}

#[test]
fn element_vec_and_iterator_render() {
    let app = elm_magic::mount_with::<Elements>(ElementsProps {
        items: Some(vec![elm_magic::ui! { <Text>"boxed"</Text> }]),
        labels: Some(vec!["x".to_string(), "y".to_string()]),
        ..Default::default()
    });
    app.expect_text("boxed");
    app.expect_text("x");
    app.expect_text("y");
}

// ── 조건/레이아웃 안쪽에서도 동작 ───────────────────────────
elm_magic::view! {
    fn Nested(title: String = String::from("nested")) {
        <Col>
            <If when={true}>
                <Row>{title}</Row>
            </If>
        </Col>
    }
}

#[test]
fn into_view_works_inside_if() {
    let app = elm_magic::mount!(Nested);
    app.expect_text("nested");
}

// ── 기존 문법 회귀 ──────────────────────────────────────────
elm_magic::view! {
    fn Legacy(items: Vec<String> = vec![], on = false) {
        <Col>
            {if on { <Text>"on"</Text> } else { <Text>"off"</Text> }}
            {items.iter().map(|i| <Row>"{i}"</Row>)}
        </Col>
    }
}

#[test]
fn legacy_forms_still_work() {
    let app = elm_magic::mount!(Legacy);
    app.expect_text("off");

    let app = elm_magic::mount_with::<Legacy>(LegacyProps {
        items: Some(vec!["a".to_string(), "b".to_string()]),
        on: Some(true),
        ..Default::default()
    });
    app.expect_text("on");
    app.expect_text("a");
    app.expect_text("b");
}
