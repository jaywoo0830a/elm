// v0.4 — 매크로 문법 픽스 (사양서 3.2, 3.3, 3.4)
//
// 1) `..` 범위 뒤 상태 읽기   2) 이벤트 본문의 상태 메서드   3) 컴마 다중문
// 4) 구조체 리터럴 축약       5) if/else·match 분기 타입 통일
// 6) 리스트 아이템 이벤트 캡처 7) `.iter()` / `.map()` 소유 반복

#[derive(Clone, PartialEq)]
struct Note {
    body: String,
}

#[derive(Clone)]
#[allow(dead_code)]
enum Status {
    Idle,
    Loading,
    Failed(String),
}

// ── 1. `..` 범위 뒤 상태 읽기 (jsx.rs `is_field_access`) ────
elm_magic::view! {
    fn Rows(n = 3) {
        <Col>
            {(0..n).map(|i| <Row>"{i}"</Row>)}
            "n = {n}"
        </Col>
    }
}

#[test]
fn range_operator_then_state_read() {
    let app = elm_magic::mount!(Rows);
    app.assert_text("0");
    app.assert_text("2");
    app.assert_text("n = 3");
}

// ── 2. 이벤트 본문의 상태 메서드 (0-인자 / 반환값 무시) ─────
fn submit(_a: String, _b: String) {}

elm_magic::view! {
    fn Signup(name = String::new(), email = String::new(), sent = false) {
        <Col>
            <Input value={name.clone()} on_change={name = _} />
            <Button on_click={submit(name.clone(), email.clone()); sent = true}>"Sign up"</Button>
            {if sent { <Text>"sent"</Text> } else { <Text>"idle"</Text> }}
        </Col>
    }
}

#[test]
fn event_body_state_method_calls() {
    let mut app = elm_magic::mount!(Signup);
    app.type_("elm");
    app.assert_text("idle");
    app.click("Sign up");
    app.assert_text("sent");
}

// ── 3. 컴마 다중문 + 값 반환 메서드 ─────────────────────────
elm_magic::view! {
    fn Notes(items: Vec<Note> = vec![], last = String::new(), body = String::new()) {
        <Col>
            <Input value={body.clone()} on_change={body = _} on_enter={items.push(Note { body })} />
            <Button on_click={items.push(Note { body }), last = String::from("added")}>"add"</Button>
            <Button on_click={items.remove(0)}>"pop"</Button>
            {items.map(|n| <Row><Button on_click={items.remove(n)}>"{n.body}"</Button></Row>)}
            "last: {last}"
        </Col>
    }
}

#[test]
fn comma_statements_and_value_returning_method() {
    let mut app = elm_magic::mount!(Notes);
    app.type_("buy milk");
    app.press_enter();
    app.assert_text("buy milk");
    // 컴마 다중문: push + last 대입
    app.type_("coffee");
    app.click("add");
    app.assert_text("added");
    app.assert_text("coffee");
    // `items.remove(0)` — 인덱스 삭제(반환값 무시)가 컴파일·동작한다
    app.click("pop");
    app.assert_hidden("buy milk");
}

// ── 4. 구조체 리터럴 축약 `Note { body }` + 요소로 삭제 ─────
#[test]
fn struct_shorthand_and_remove_by_element() {
    let mut app = elm_magic::mount!(Notes);
    app.type_("alpha");
    app.press_enter();
    app.type_("beta");
    app.press_enter();
    app.assert_text("alpha");
    app.assert_text("beta");
    // 요소로 삭제 (사양서 3.3의 `items.remove(t)`)
    app.click("alpha");
    app.assert_hidden("alpha");
    app.assert_text("beta");
}

// ── 5. if/else · match 분기 타입 통일 (사양서 3.4) ──────────
elm_magic::view! {
    fn Feed(
        loading = true,
        items: Vec<String> = vec![],
        status: Status = Status::Idle,
    ) {
        <Col>
            {if loading { <Spinner /> } else { items.iter().map(|i| <Row>"{i}"</Row>) }}
            {match status {
                Status::Idle => <Text>"ready"</Text>,
                Status::Loading => <Spinner />,
                Status::Failed(e) => <Banner kind="error">{e}</Banner>,
            }}
            {if items.is_empty() { "empty" } else { "full" }}
        </Col>
    }
}

#[test]
fn conditional_branches_unify_element_and_iterator() {
    let app = elm_magic::mount!(Feed);
    app.assert_text("[spinner]");
    app.assert_text("ready");
    app.assert_text("empty");
}

#[test]
fn match_arms_unify_with_banner_children() {
    let app = elm_magic::mount_with::<Feed>(FeedProps {
        loading: Some(false),
        items: Some(vec!["a".to_string(), "b".to_string()]),
        status: Some(Status::Failed("boom".to_string())),
        ..Default::default()
    });
    app.assert_text("a");
    app.assert_text("b");
    app.assert_text("boom");
    app.assert_text("full");
    app.assert_hidden("[spinner]");
}

// ── 6. 리스트 아이템을 이벤트에서 캡처 (E0716 해결) ─────────
elm_magic::view! {
    fn Picker(items: Vec<String> = vec![], selected = String::new()) {
        <Col>
            {items.map(|t| <Row><Button on_click={selected = t.clone()}>"{t}"</Button></Row>)}
            "sel: {selected}"
        </Col>
    }
}

#[test]
fn list_item_captured_in_event_handler() {
    let mut app = elm_magic::mount_with::<Picker>(PickerProps {
        items: Some(vec!["a".to_string(), "b".to_string()]),
        selected: Some(String::new()),
        ..Default::default()
    });
    app.click("b");
    app.assert_text("sel: b");
}

// ── 7. `.iter()` / `.map()` 소유 반복 (사양서 3.3) ──────────
elm_magic::view! {
    fn Sum(values: Vec<f64> = vec![]) {
        let doubled = values.iter().map(|v| v * 2.0).sum::<f64>();
        <Col>
            {values.iter().map(|v| <Row>"{v}"</Row>)}
            "doubled: {doubled}"
        </Col>
    }
}

#[test]
fn iter_sugar_yields_owned_items() {
    let app = elm_magic::mount_with::<Sum>(SumProps {
        values: Some(vec![1.0, 2.0]),
        ..Default::default()
    });
    app.assert_text("1");
    app.assert_text("2");
    app.assert_text("doubled: 6");
}

// ── 뷰 본문 전체가 조건부일 때 (Fragment 마감) ──────────────
elm_magic::view! {
    fn Whole(loading = true) {
        {if loading { <Spinner /> } else { <Text>"done"</Text> }}
    }
}

#[test]
fn body_may_be_a_whole_conditional() {
    let app = elm_magic::mount!(Whole);
    app.assert_text("[spinner]");
    let app = elm_magic::mount_with::<Whole>(WholeProps {
        loading: Some(false),
        ..Default::default()
    });
    app.assert_text("done");
}
