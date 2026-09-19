//! 위젯 프로토콜 표 — 17개 빌트인 태그가 `Widget` 트레이트에 답하는 값을 **전부** 고정한다.
//!
//! `tests/widget_protocol.rs`가 프로토콜의 *사용법*을 검증한다면, 이 파일은 그
//! **표 자체**(kind/tag/role/interactive/disabled/label/children)를 못 박는다.
//! 새 태그를 추가하거나 role을 바꾸면 이 표가 먼저 깨진다.
//!
//! 표는 `src/`를 읽지 않고 공개 API(`Element`의 프로토콜 메서드)를 관측해 채웠다.

use elm_magic::prelude::*;

elm_magic::view! {
    fn Matrix(
        text = String::new(),
        checked = false,
        open = false,
        tab = 0,
    ) {
        <Col>
            <Row>"row"</Row>
            <Text class="t">"text"</Text>
            <Strong>"strong"</Strong>
            <Button>"button"</Button>
            <Input value={text.clone()} on_change={text = _} />
            <TextArea value={text.clone()} on_change={text = _} />
            <Check checked={checked} on_change={checked = _}>"check"</Check>
            <Tab active={tab == 0} on_click={tab = 0}>"tab"</Tab>
            <Th on_click={tab = 1}>"th"</Th>
            <Td>"td"</Td>
            <Banner kind="error">"banner"</Banner>
            <Spinner />
            <Divider />
            <Progress value={0.5} />
            <Modal on_close={open = false}><Text>"modal"</Text></Modal>
            <Raw>|_ui: &mut ()| {}</Raw>
        </Col>
    }
}

#[derive(Debug, PartialEq)]
struct Node<'a> {
    kind: &'a str,
    tag: &'a str,
    role: Role,
    interactive: bool,
    disabled: bool,
    label: Option<&'a str>,
    children: Option<usize>,
}

fn collect<'a>(el: &'a Element, out: &mut Vec<Node<'a>>) {
    out.push(Node {
        kind: el.kind(),
        tag: el.tag(),
        role: el.role(),
        interactive: el.is_interactive(),
        disabled: el.is_disabled(),
        label: el.label(),
        children: el.children().map(|c| c.len()),
    });
    for child in el.children().unwrap_or(&[]) {
        collect(child, out);
    }
}

/// 살아 있는 트리 위에서만 단언할 수 있도록 스코프를 열어준다 (Node가 Element를 빌린다).
fn with_matrix<R>(f: impl FnOnce(&[Node<'_>]) -> R) -> R {
    let app = elm_magic::mount!(Matrix);
    let mut nodes = Vec::new();
    collect(app.element(), &mut nodes);
    f(&nodes)
}

/// (kind, tag, role, interactive, disabled, label, children) 한 줄.
type Row7 = (
    &'static str,
    &'static str,
    Role,
    bool,
    bool,
    Option<&'static str>,
    Option<usize>,
);

// 깊이 우선(전위) 순회. Col 아래 16개 + Modal 안의 Text 1개.
const EXPECTED: [Row7; 19] = [
    ("Col", "col", Role::Group, false, false, None, Some(16)),
    ("Row", "row", Role::Group, false, false, None, Some(1)),
    ("Text", "text", Role::Text, false, false, Some("row"), None),
    ("Text", "text", Role::Text, false, false, Some("text"), None),
    (
        "Strong",
        "strong",
        Role::Strong,
        false,
        false,
        Some("strong"),
        None,
    ),
    (
        "Button",
        "button",
        Role::Button,
        true,
        false,
        Some("button"),
        None,
    ),
    ("Input", "input", Role::TextBox, true, false, Some(""), None),
    (
        "TextArea",
        "textarea",
        Role::TextBox,
        true,
        false,
        Some(""),
        None,
    ),
    (
        "Check",
        "check",
        Role::Checkbox,
        true,
        false,
        Some("check"),
        None,
    ),
    ("Tab", "tab", Role::Tab, true, false, Some("tab"), None),
    (
        "Th",
        "th",
        Role::ColumnHeader,
        true,
        false,
        Some("th"),
        None,
    ),
    ("Td", "td", Role::Cell, false, false, Some("td"), None),
    (
        "Banner",
        "banner",
        Role::Banner,
        false,
        false,
        Some("banner"),
        None,
    ),
    (
        "Spinner",
        "spinner",
        Role::Spinner,
        false,
        false,
        None,
        None,
    ),
    (
        "Divider",
        "divider",
        Role::Separator,
        false,
        false,
        None,
        None,
    ),
    (
        "Progress",
        "progress",
        Role::Progress,
        false,
        false,
        None,
        None,
    ),
    (
        "Modal",
        "modal",
        Role::Dialog,
        false,
        false,
        Some(""),
        Some(1),
    ),
    (
        "Text",
        "text",
        Role::Text,
        false,
        false,
        Some("modal"),
        None,
    ),
    ("Raw", "raw", Role::Raw, false, false, None, None),
];

#[test]
fn protocol_table_is_pinned() {
    with_matrix(|nodes| {
        assert_eq!(nodes.len(), EXPECTED.len(), "노드 수가 다르다: {nodes:#?}");
        for (node, expected) in nodes.iter().zip(EXPECTED.iter()) {
            assert_eq!(node.kind, expected.0, "kind 불일치: {node:?}");
            assert_eq!(node.tag, expected.1, "tag 불일치: {node:?}");
            assert_eq!(&node.role, &expected.2, "role 불일치: {node:?}");
        }
    });
}

#[test]
fn interactivity_table_is_pinned() {
    with_matrix(|nodes| {
        for (node, expected) in nodes.iter().zip(EXPECTED.iter()) {
            assert_eq!(
                node.interactive, expected.3,
                "{}의 is_interactive()가 표와 다르다",
                node.kind
            );
            assert_eq!(
                node.disabled, expected.4,
                "{}의 is_disabled()가 표와 다르다",
                node.kind
            );
        }
    });
}

#[test]
fn label_and_children_table_is_pinned() {
    with_matrix(|nodes| {
        for (node, expected) in nodes.iter().zip(EXPECTED.iter()) {
            assert_eq!(
                node.label, expected.5,
                "{}의 label()이 표와 다르다",
                node.kind
            );
            assert_eq!(
                node.children, expected.6,
                "{}의 children()이 표와 다르다",
                node.kind
            );
        }
    });
}

#[test]
fn only_button_input_textarea_check_tab_th_are_interactive() {
    with_matrix(|nodes| {
        let interactive: Vec<&str> = nodes
            .iter()
            .filter(|n| n.interactive)
            .map(|n| n.kind)
            .collect();
        assert_eq!(
            interactive,
            ["Button", "Input", "TextArea", "Check", "Tab", "Th"],
            "상호작용 가능한 태그 목록이 바뀌었다"
        );
    });
}

#[test]
fn containers_have_children_and_leaves_do_not() {
    with_matrix(|nodes| {
        for node in nodes {
            if node.children.is_some() {
                assert!(
                    matches!(node.role, Role::Group | Role::Dialog),
                    "자식을 가진 것은 컨테이너뿐이어야 한다: {node:?}"
                );
            }
        }
    });
}

// ── Fragment는 태그도 role도 없다 ───────────────────────────

#[test]
fn fragment_is_not_a_tag() {
    let frag = elm_magic::into_element(vec![
        elm_magic::ui! { <Text>"a"</Text> },
        elm_magic::ui! { <Text>"b"</Text> },
    ]);
    assert_eq!(frag.kind(), "Fragment");
    assert_eq!(frag.tag(), "", "Fragment는 태그 셀렉터와 매치되지 않는다");
    assert_eq!(frag.role(), Role::None);
    assert!(!frag.is_interactive());
    assert_eq!(frag.children().map(|c| c.len()), Some(2));
}

// ── 접근성 트리는 role 이름을 그대로 쓴다 ───────────────────

#[test]
fn a11y_tree_uses_the_role_names() {
    let app = elm_magic::mount!(Matrix);
    let tree = app.a11y_tree();
    for expected in [
        "group",
        "text \"row\"",
        "strong \"strong\"",
        "button \"button\"",
        "textbox \"\"",
        "checkbox \"check\"",
        "tab \"tab\"",
        "columnheader \"th\"",
        "cell \"td\"",
        "banner \"banner\"",
        "spinner",
        "separator",
        "progressbar",
        "dialog \"\"",
        "raw",
    ] {
        assert!(tree.contains(expected), "{expected:?}가 없다:\n{tree}");
    }
}

// ── 조용히 버려지는 속성 — 현재 의미론 ─────────────────────

elm_magic::view! {
    fn MatrixIgnored(done = true) {
        <Col>
            <Text strike={done} draggable={done}>"plain"</Text>
            <Row highlight={done}><Text>"row"</Text></Row>
        </Col>
    }
}

#[test]
fn ignored_attributes_do_not_reach_the_tree() {
    // 현황 문서 3.2: `strike` / `role` / `draggable` / `highlight` / `accept`는
    // 오류 없이 **조용히 버려진다**. 실제 prop이 생기면 이 테스트를 뒤집는다.
    let app = elm_magic::mount!(MatrixIgnored);
    let tree = app.render_tree();
    assert!(tree.contains("Text \"plain\""), "tree:\n{tree}");
    for leak in ["strike", "draggable", "highlight", "auto_focus", "accept"] {
        assert!(!tree.contains(leak), "{leak}가 트리에 나타났다:\n{tree}");
    }
}

// ── 문서와 실제가 다른 지점: `class`가 어디서 무시되는가 ────

elm_magic::css! {
    .mx_cls { gap: 5; }
}

#[test]
fn class_on_input_and_textarea_is_stored_and_resolved() {
    // 현황 문서 2.9는 "`class` on `Input` / `TextArea` / `Raw` is ignored"라고 적고
    // 있지만, 실측으로는 **매크로가 저장하고 코어가 해석한다**(`class()` /
    // `resolved_style()`). 무시되는 지점은 **어댑터가 그 스타일을 위젯에 입히지 않는
    // 것**이므로, 원인 위치가 문서와 다르다 (core ↔ adapter 드리프트).
    // 참고: `<Raw class="…">`는 아예 **컴파일 에러**다 (Raw는 클로저 자식만 받는다).
    let input = elm_magic::ui! { <Input class="mx_cls" /> };
    assert_eq!(input.class().to_vec(), ["mx_cls"]);
    assert_eq!(
        input.resolved_style(&elm_magic::style::Palette::dark()).gap,
        Some(5.0),
        "코어는 클래스를 해석한다"
    );

    let area = elm_magic::ui! { <TextArea class="mx_cls" /> };
    assert_eq!(area.class().to_vec(), ["mx_cls"]);
}
