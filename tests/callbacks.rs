//! v0.5 — 콜백 prop + 사용자 컴포넌트 children (사양서 3.1)
//!
//! - `fn ItemRow(item: Item, on_select: fn(Id))` — 부모에게 값을 올려보내는 콜백
//! - `<ItemRow item={i} on_select={selected = Some(_)} />` — 호출부에서 클로저 생성
//! - `<Card>…</Card>` — 자식 컴포넌트가 `{children}`으로 자식을 받는다

use elm_magic::prelude::*;

#[derive(Clone, PartialEq)]
struct Item {
    id: i32,
    name: String,
}

// ── 콜백 prop (사양서 1.rs 패턴 7) ──────────────────────────

elm_magic::view! {
    fn ItemRow(item: Item, on_select: fn(i32)) {
        <Row on_click={on_select(item.id)}>
            "{item.name}"
        </Row>
    }
}

elm_magic::view! {
    fn List(items: Vec<Item> = vec![], selected = 0) {
        <Col>
            {items.map(|i| <ItemRow item={i} on_select={selected = _} />)}
            "selected: {selected}"
        </Col>
    }
}

#[test]
fn callback_prop_calls_back_into_parent() {
    let mut app = elm_magic::mount_with::<List>(ListProps {
        items: Some(vec![
            Item {
                id: 7,
                name: "seven".to_string(),
            },
            Item {
                id: 8,
                name: "eight".to_string(),
            },
        ]),
        ..Default::default()
    });
    app.assert_text("selected: 0");
    app.click("eight");
    app.assert_text("selected: 8");
    app.click("seven");
    app.assert_text("selected: 7");
}

// 콜백이 부모 상태를 바꾸는 형태 (`on_select={selected = Some(_)}`의 축약)
elm_magic::view! {
    fn Picker(items: Vec<Item> = vec![], last = String::new()) {
        <Col>
            {items.map(|i| <ItemRow item={i} on_select={last = format!("#{}", _)} />)}
            "last: {last}"
        </Col>
    }
}

#[test]
fn callback_can_use_the_incoming_value() {
    let mut app = elm_magic::mount_with::<Picker>(PickerProps {
        items: Some(vec![Item {
            id: 3,
            name: "three".to_string(),
        }]),
        ..Default::default()
    });
    app.click("three");
    app.assert_text("last: #3");
}

// 콜백 prop에 기본값 없이도 잘 컴파일되는지 (prop 누락 시 panic 메시지)
#[test]
fn required_prop_missing_panics_with_message() {
    let result = std::panic::catch_unwind(|| {
        let _ = elm_magic::mount!(ItemRow);
    });
    assert!(result.is_err(), "필수 prop `item` 없이 마운트하면 panic");
}

// ── 사용자 컴포넌트 children (사양서 3.1) ───────────────────

elm_magic::view! {
    fn Card(title = String::new()) {
        <Col class="card">
            "card: {title}"
            {children}
        </Col>
    }
}

elm_magic::view! {
    fn Page(n = 0) {
        <Col>
            <Card title="hello">
                <Text>"body {n}"</Text>
                <Button on_click={n += 1}>"inc"</Button>
            </Card>
        </Col>
    }
}

#[test]
fn component_children_are_rendered() {
    let mut app = elm_magic::mount!(Page);
    app.assert_text("card: hello");
    app.assert_text("body 0");
    // 자식 요소의 핸들러도 부모 상태를 바꾼다
    app.click("inc");
    app.assert_text("body 1");
}

#[test]
fn component_without_children_still_renders() {
    let app = elm_magic::mount!(Card);
    app.assert_text("card: ");
}

// children과 일반 prop을 함께 쓰는 중첩
elm_magic::view! {
    fn Badge(label = String::new()) {
        <Row>"[{label}]"</Row>
    }
}

elm_magic::view! {
    fn Nested() {
        <Card title="outer">
            <Badge label="inner" />
            <Text>"leaf"</Text>
        </Card>
    }
}

#[test]
fn nested_components_in_children() {
    let app = elm_magic::mount!(Nested);
    app.assert_text("[inner]");
    app.assert_text("leaf");
}
