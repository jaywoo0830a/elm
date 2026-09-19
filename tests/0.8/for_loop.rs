//! 0.8.0 반복 태그 — `<For>` (사양: `0.8-preview.md` §3.1).
//!
//! 계약:
//! - `each={..}`는 `IntoIterator`, `as={name}`은 아이템 바인딩 (본문·이벤트·보간에서 사용).
//! - `key`가 없으면 **위치 기반**, 있으면 **keyed 슬롯** — 자식 상태가 키를 따라간다
//!   (0.7의 key 규약 그대로).
//! - `<For>`는 레이아웃 노드를 만들지 않는다 (아이템 요소가 그대로 자식이 된다).
//! - 빈 컬렉션/빈 범위는 아무것도 그리지 않는다.
//!
//! 이 파일은 `cargo test --test v0_8_for_loop`로 돈다.

#[derive(Clone, PartialEq, Debug)]
struct Todo {
    id: u64,
    text: String,
}

fn todo(id: u64, text: &str) -> Todo {
    Todo {
        id,
        text: text.to_string(),
    }
}

// ── 기본: 순서 보존 + 아이템 바인딩 ─────────────────────────
elm_magic::view! {
    fn List(items: Vec<Todo> = vec![], selected = String::new()) {
        <Col>
            <For each={items} as={t}>
                <Row>
                    <Button on_click={selected = t.text.clone()}>"{t.text}"</Button>
                </Row>
            </For>
            "sel: {selected}"
        </Col>
    }
}

#[test]
fn for_renders_items_in_order() {
    let app = elm_magic::mount_with::<List>(ListProps {
        items: Some(vec![todo(1, "a"), todo(2, "b"), todo(3, "c")]),
        ..Default::default()
    });

    let all = app.text();
    let a = all.find('a').expect("a");
    let b = all.find('b').expect("b");
    let c = all.find('c').expect("c");
    assert!(a < b && b < c, "order must be preserved: {all:?}");
}

#[test]
fn for_binds_item_in_event_handler() {
    let mut app = elm_magic::mount_with::<List>(ListProps {
        items: Some(vec![todo(1, "a"), todo(2, "b")]),
        ..Default::default()
    });
    app.click("b");
    app.expect_text("sel: b");
}

#[test]
fn for_is_transparent_no_wrapper_node() {
    let app = elm_magic::mount_with::<List>(ListProps {
        items: Some(vec![todo(1, "a"), todo(2, "b")]),
        ..Default::default()
    });
    // 아이템 2개 + 마지막 "sel: …" 텍스트 = 3. `<For>` 래퍼가 있으면 2가 된다.
    let children = app.element().children().expect("Col has children");
    assert_eq!(
        children.len(),
        3,
        "<For> must not add a wrapper node:\n{}",
        app.render_tree()
    );
}

#[test]
fn for_empty_renders_nothing() {
    let app = elm_magic::mount!(List);
    let children = app.element().children().expect("Col has children");
    assert_eq!(children.len(), 1, "only the trailing text remains");
    app.expect_text("sel: ");
}

// ── 이터레이터 (`each`는 IntoIterator) ──────────────────────
elm_magic::view! {
    fn Countdown(n = 3) {
        <Col>
            <For each={0..n} as={i}>
                <Text>"{i}"</Text>
            </For>
        </Col>
    }
}

#[test]
fn for_accepts_arbitrary_iterator() {
    let app = elm_magic::mount!(Countdown);
    app.expect_text("0");
    app.expect_text("1");
    app.expect_text("2");
    app.assert_hidden("3");
}

#[test]
fn for_over_empty_range_renders_nothing() {
    let app = elm_magic::mount_with::<Countdown>(CountdownProps {
        n: Some(0),
        ..Default::default()
    });
    let children = app.element().children().expect("Col has children");
    assert_eq!(children.len(), 0);
}

// ── 중첩: `<For>` 안의 `<If>` ───────────────────────────────
elm_magic::view! {
    fn Filtered(items: Vec<Todo> = vec![]) {
        <Col>
            <For each={items} as={t}>
                <Row>
                    <Text>"{t.text}"</Text>
                    <If when={t.text.len() > 1}>
                        <Text>"long"</Text>
                    </If>
                </Row>
            </For>
        </Col>
    }
}

#[test]
fn if_inside_for_uses_item_binding() {
    let app = elm_magic::mount_with::<Filtered>(FilteredProps {
        items: Some(vec![todo(1, "a"), todo(2, "bb")]),
        ..Default::default()
    });
    assert_eq!(
        app.text().matches("long").count(),
        1,
        "only the multi-char item is long:\n{}",
        app.text()
    );
}

// ── keyed vs 위치 기반: 자식 상태가 키를 따라가는가 ─────────
elm_magic::view! {
    fn KeyedCounter(label = String::new(), n = 0) {
        <Row>
            <Button on_click={n += 1}>"{label}+"</Button>
            "{label}={n}"
        </Row>
    }
}

elm_magic::view! {
    fn KeyedList(items: Vec<Todo> = vec![]) {
        <Col>
            <Button on_click={items.remove(0)}>"remove-first"</Button>
            <For each={items} as={t} key={t.id}>
                <KeyedCounter label={t.text.clone()} />
            </For>
        </Col>
    }
}

elm_magic::view! {
    fn PositionalList(items: Vec<Todo> = vec![]) {
        <Col>
            <Button on_click={items.remove(0)}>"remove-first"</Button>
            <For each={items} as={t}>
                <KeyedCounter label={t.text.clone()} />
            </For>
        </Col>
    }
}

fn three_items() -> Vec<Todo> {
    vec![todo(1, "a"), todo(2, "b"), todo(3, "c")]
}

#[test]
fn keyed_for_preserves_child_state_by_key() {
    let mut app = elm_magic::mount_with::<KeyedList>(KeyedListProps {
        items: Some(three_items()),
        ..Default::default()
    });

    app.click("b+");
    app.expect_text("b=1");

    // 첫 아이템을 지워도 b의 카운터 상태는 키(b.id)를 따라 유지된다.
    app.click("remove-first");
    app.expect_text("b=1");
    app.assert_hidden("a=0");
}

#[test]
fn positional_for_does_not_follow_key() {
    let mut app = elm_magic::mount_with::<PositionalList>(PositionalListProps {
        items: Some(three_items()),
        ..Default::default()
    });

    app.click("b+");
    app.expect_text("b=1");

    // 위치 기반이면 b가 앞으로 밀리며 상태가 키를 따라가지 않는다.
    app.click("remove-first");
    app.assert_hidden("b=1");
}
