#[derive(Clone)]
struct Todo {
    text: String,
    #[allow(dead_code)]
    done: bool,
}

// 사양서 3.3 — 리스트 + 입력
elm_magic::view! {
    fn Todos(items: Vec<Todo> = vec![], text = String::new()) {
        <Col>
            <Input value={text.clone()} on_change={text = _} on_enter={items.push(Todo { text: text.clone(), done: false })} />
            {items.map(|t| <Row>"{t.text}"</Row>)}
        </Col>
    }
}

// 사양서 3.4 — 조건부 + 파생 값
elm_magic::view! {
    fn Cart(items: Vec<(String, f64)> = vec![]) {
        let total = items.iter().map(|i| i.1).sum::<f64>();
        <Col>
            {items.map(|i| <Row>"{i.0}"</Row>)}
            {if total > 0.0 {
                "Subtotal: {total}"
            } else {
                "Empty: {items.len()}"
            }}
        </Col>
    }
}

#[test]
fn todo_add_via_enter() {
    let mut app = elm_magic::mount!(Todos);
    app.type_("buy milk");
    app.press_enter();
    app.expect_text("buy milk");
}

#[test]
fn todo_add_two_items() {
    let mut app = elm_magic::mount!(Todos);
    app.type_("a");
    app.press_enter();
    app.type_("b");
    app.press_enter();
    app.expect_text("a");
    app.expect_text("b");
    assert!(app.text().matches("a").count() >= 1);
}

#[test]
fn cart_conditional_and_derived() {
    let app = elm_magic::mount_with::<Cart>(CartProps {
        items: vec![("A".into(), 10.0), ("B".into(), 5.0)],
    });
    app.expect_text("Subtotal: 15");
    app.expect_text("A");
    app.expect_text("B");
}

#[test]
fn cart_empty() {
    let app = elm_magic::mount!(Cart);
    app.expect_text("Empty: 0");
}
