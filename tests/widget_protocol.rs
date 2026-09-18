//! v0.7 Widget Protocol — `enum_dispatch` 기반 위젯 프로토콜 검증.
//!
//! 0.6까지 소비 측(테스트/접근성/어댑터)은 `Element`의 variant를 직접
//! 매칭해야 했다. v0.7부터는 `Widget` 트레이트가 **한 곳**에서 위젯의
//! 능력을 답하고, `enum_dispatch`가 그 호출을 각 위젯 구조체로 위임한다.
//!
//! 이 파일은 variant 매칭 **없이** 다음이 가능함을 고정한다:
//! - role/label/is_interactive/is_disabled 같은 프로토콜 질의
//! - role 기반 테스트 셀렉터 (`click_role`, `sel!`)
//! - 접근성 트리 (`a11y_tree`, `roles`, `has_role`)
//! - 트레이트 기반 이벤트 접근 (`on_click`, `on_value_change`, `on_bool_change`)

use elm_magic::prelude::*;
use elm_magic::sel;

// ── 프로토콜 질의: variant를 몰라도 위젯의 능력을 묻는다 ──────

#[test]
fn element_answers_widget_protocol_without_variant_matching() {
    let el = elm_magic::ui! { <Button disabled={true}>"go"</Button> };

    assert_eq!(el.kind(), "Button");
    assert_eq!(el.tag(), "button");
    assert_eq!(el.role(), Role::Button);
    assert_eq!(el.label(), Some("go"));
    assert!(el.is_interactive());
    assert!(el.is_disabled());
    assert!(el.on_click().is_none(), "핸들러를 주지 않았다");
}

#[test]
fn text_widget_protocol() {
    let el = elm_magic::ui! { <Text class="muted">"hi"</Text> };
    assert_eq!(el.role(), Role::Text);
    assert_eq!(el.label(), Some("hi"));
    assert_eq!(el.class().to_vec(), ["muted"]);
    assert!(!el.is_interactive());
}

#[test]
fn input_widget_protocol() {
    let el = elm_magic::ui! { <Input value={"typed".to_string()} /> };
    assert_eq!(el.role(), Role::TextBox);
    assert_eq!(el.value(), Some("typed"));
    assert!(el.is_interactive());
}

#[test]
fn raw_widget_exposes_the_opaque_closure() {
    let el = elm_magic::ui! { <Raw>|_ui: &mut ()| {}</Raw> };
    assert_eq!(el.role(), Role::Raw);
    assert!(el.raw_fn().is_some(), "어댑터가 쓸 불투명 클로저");
    assert_eq!(el.tag(), "raw");
}

#[test]
fn fragment_has_no_tag_and_role_is_none() {
    // 분기 마감으로 생기는 Fragment는 태그도 role도 없다.
    let el = elm_magic::into_element(vec![
        elm_magic::ui! { <Text>"a"</Text> },
        elm_magic::ui! { <Text>"b"</Text> },
    ]);
    assert_eq!(el.kind(), "Fragment");
    assert_eq!(el.tag(), "");
    assert_eq!(el.role(), Role::None);
    assert_eq!(el.children().map(|c| c.len()), Some(2));
}

// ── role 기반 테스트 셀렉터 ────────────────────────────────

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
fn click_role_finds_by_role_and_label() {
    let mut app = elm_magic::mount!(Counter);
    app.click_role(Role::Button, "+");
    app.expect_text("Count: 1");
    app.click_role(Role::Button, "-");
    app.expect_text("Count: 0");
}

#[test]
fn sel_macro_selects_role_tag_class() {
    let mut app = elm_magic::mount!(Counter);

    assert!(app.exists(&sel!(role Button, "+")));
    assert!(app.exists(&sel!(role Button)));
    assert!(app.exists(&sel!(tag button)));
    assert!(app.exists(&sel!(tag Row)));
    assert!(!app.exists(&sel!(class nonexistent)));

    // role Button 중 첫 번째("-")를 클릭한다
    app.click_sel(&sel!(role Button));
    app.expect_text("Count: -1");

    // 라벨까지 지정
    app.click_sel(&sel!(role Button, "+"));
    app.expect_text("Count: 0");
}

#[test]
fn selector_builder_composes() {
    let app = elm_magic::mount!(Counter);
    let by_class = Selector::class("nope").and_role(Role::Button);
    assert!(!app.exists(&by_class));
    // tag + role 조합: Button 중 tag가 button인 것
    let combo = Selector::role(Role::Button).and_tag("button");
    assert!(app.exists(&combo));
}
// ── 접근성 트리 ────────────────────────────────────────────

#[test]
fn a11y_tree_lists_roles_and_labels() {
    let app = elm_magic::mount!(Counter);
    let tree = app.a11y_tree();
    assert!(tree.contains("group"), "루트 Row는 group:\n{tree}");
    assert!(tree.contains("button \"+\""), "tree:\n{tree}");
    assert!(tree.contains("button \"-\""), "tree:\n{tree}");
}

#[test]
fn roles_are_enumerable() {
    let app = elm_magic::mount!(Counter);
    let roles = app.roles();
    assert!(roles
        .iter()
        .any(|(r, l)| *r == Role::Button && l.as_deref() == Some("+")));
    assert!(app.has_role(Role::Group));
    assert!(app.has_role(Role::Button));
    assert!(!app.has_role(Role::Checkbox));
}

// ── 트레이트 기반 이벤트 접근 ──────────────────────────────

elm_magic::view! {
    fn Form(name = String::new(), done = false) {
        <Col>
            <Input value={name.clone()} on_change={name = _} on_enter={name = String::from("sent")} />
            <Check checked={done} on_change={done = _}>"done"</Check>
            "name: {name}"
        </Col>
    }
}

fn find_role<'a>(el: &'a Element, role: Role) -> Option<&'a Element> {
    if el.role() == role {
        return Some(el);
    }
    for c in el.children().unwrap_or(&[]) {
        if let Some(found) = find_role(c, role) {
            return Some(found);
        }
    }
    None
}

#[test]
fn handlers_are_reachable_through_the_trait() {
    let app = elm_magic::mount!(Form);
    let input = find_role(app.element(), Role::TextBox).expect("input");
    assert!(input.on_value_change().is_some());
    assert!(input.on_value_enter().is_some());
    assert_eq!(input.value(), Some(""));

    let check = find_role(app.element(), Role::Checkbox).expect("check");
    assert!(check.on_bool_change().is_some());
    assert_eq!(check.checked(), Some(false));
    assert_eq!(check.label(), Some("done"));
}

#[test]
fn input_enter_and_check_still_work() {
    let mut app = elm_magic::mount!(Form);
    app.type_("rust");
    app.expect_text("name: rust");
    app.press_enter();
    app.expect_text("name: sent");

    app.toggle("done");
    app.assert_text("name: sent");
}

// ── 컨테이너도 프로토콜을 따른다 ────────────────────────────

elm_magic::view! {
    fn ClickableRow(hits = 0) {
        <Row on_click={hits += 1}>"hits {hits}"</Row>
    }
}

#[test]
fn container_on_click_is_interactive() {
    let mut app = elm_magic::mount!(ClickableRow);
    assert_eq!(app.element().role(), Role::Group);
    assert!(app.element().is_interactive());
    assert!(app.element().on_click().is_some());
    app.click("hits 0");
    app.expect_text("hits 1");
}

#[test]
fn selector_subtree_matches_containers() {
    let app = elm_magic::mount!(ClickableRow);
    // 컨테이너는 라벨이 아니라 서브트리 텍스트로 매칭한다
    assert!(app.exists(&Selector::subtree("hits 0")));
    assert!(app.exists(&Selector::subtree("hits 0").and_role(Role::Group)));
    assert!(!app.exists(&Selector::subtree("hits 0").and_role(Role::Button)));
    assert!(!app.exists(&Selector::subtree("nope")));
}
