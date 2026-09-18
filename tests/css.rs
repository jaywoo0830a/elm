// v0.2 — css! 등록/조회 (사양서 6.1)
// v0.6 — 컴파일타임 검증 + 클래스/태그 셀렉터 정합성

elm_magic::css! {
    .app { gap: 16; padding: 24; }
    .card { gap: 8; padding: 16; bg: surface; radius: 8; }
    .muted { color: text_dim; }
    button { bg: primary; color: on_primary; padding: 8 16; radius: 6; }
    // 태그 `button`과 클래스 `.button`은 서로 다른 셀렉터다 (키 공간 분리)
    .button { bg: warn; }
}

// 팔레트 토큰 14종 — `Token::ALL`과 어긋나면
// `css_token_vocabulary_matches_core`가 잡는다.
elm_magic::css! {
    .tok_primary { bg: primary; }
    .tok_on_primary { bg: on_primary; }
    .tok_surface { bg: surface; }
    .tok_surface_alt { bg: surface_alt; }
    .tok_background { bg: background; }
    .tok_text { bg: text; }
    .tok_text_dim { bg: text_dim; }
    .tok_error { bg: error; }
    .tok_warn { bg: warn; }
    .tok_success { bg: success; }
    .tok_info { bg: info; }
    .tok_border { bg: border; }
    .tok_shadow { bg: shadow; }
    .tok_overlay { bg: overlay; }
}

#[test]
fn css_registers_classes() {
    let card = elm_magic::style::lookup(".card").expect(".card registered");
    assert_eq!(card.get("gap").as_deref(), Some("8"));
    assert_eq!(card.get("padding").as_deref(), Some("16"));
    assert_eq!(card.get("bg").as_deref(), Some("surface"));
    assert_eq!(card.get("radius").as_deref(), Some("8"));
    assert_eq!(card.get("color"), None, "card has no color");
}

#[test]
fn css_registers_tag_selectors() {
    let button = elm_magic::style::lookup("button").expect("button registered");
    assert_eq!(button.get("bg").as_deref(), Some("primary"));
    assert_eq!(button.get("padding").as_deref(), Some("8 16"), "2값 숏핸드");
    assert_eq!(button.get("radius").as_deref(), Some("6"));
}

#[test]
fn css_unknown_class_is_none() {
    assert!(elm_magic::style::lookup(".nonexistent").is_none());
}

// ── v0.6 정합성: 클래스/태그 분리, 조회 규약 ──────────────

#[test]
fn css_class_and_tag_are_separate_namespaces() {
    let tag = elm_magic::style::lookup("button").expect("tag `button`");
    assert_eq!(tag.get("bg").as_deref(), Some("primary"));
    let class = elm_magic::style::lookup_class("button").expect("class `.button`");
    assert_eq!(class.get("bg").as_deref(), Some("warn"));
    assert!(
        elm_magic::style::lookup_tag("button").is_some(),
        "`button`은 태그 조회로도 찾힌다"
    );
}

#[test]
fn css_lookup_class_accepts_bare_and_dotted() {
    let dotted = elm_magic::style::lookup_class(".card").expect(".card");
    let bare = elm_magic::style::lookup_class("card").expect("card");
    assert_eq!(dotted, bare, "점은 붙여도 안 붙여도 같다");
    assert_eq!(dotted.selector(), ".card", "selector()는 쓴 그대로");
}

#[test]
fn css_tag_lookup_is_case_insensitive() {
    assert!(elm_magic::style::lookup_tag("Button").is_some());
    assert!(elm_magic::style::lookup_tag("BUTTON").is_some());
}

#[test]
fn css_token_vocabulary_matches_core() {
    use elm_magic::style::{lookup_class, Token};
    for (name, _) in Token::ALL {
        let selector = format!(".tok_{name}");
        let props = lookup_class(&selector)
            .unwrap_or_else(|| panic!("{selector} 미등록 — css!의 토큰 목록에 `{name}`이 없다"));
        assert_eq!(props.get("bg").as_deref(), Some(*name));
    }
}

#[test]
fn css_register_keeps_first_and_is_idempotent() {
    let before = elm_magic::style::len();
    elm_magic::css! {
        .card { gap: 999; }
    }
    assert_eq!(elm_magic::style::len(), before, "같은 셀렉터는 늘지 않는다");
    let card = elm_magic::style::lookup_class("card").expect("card");
    assert_eq!(card.get("gap").as_deref(), Some("8"), "첫 등록이 이긴다");
}

// ── v0.6 정합성: `class` 속성 ─────────────────────────────

#[test]
fn css_multi_class_in_one_attribute() {
    // 예전에는 `vec!["muted app"]` 하나로 들어가 어떤 셀렉터와도 안 맞았다
    let el = elm_magic::ui! { <Text class="muted app">"hi"</Text> };
    assert_eq!(el.class().to_vec(), ["muted", "app"]);
}

#[test]
fn css_conditional_class_is_a_value() {
    // 사양서 6.2 — 조건부 스타일은 값이다
    let el = elm_magic::ui! { <Text class={if true { "muted" } else { "app" }}>"hi"</Text> };
    assert_eq!(el.class().to_vec(), ["muted"]);
    let el2 = elm_magic::ui! { <Text class={if false { "muted" } else { "app" }}>"hi"</Text> };
    assert_eq!(el2.class().to_vec(), ["app"]);
}

#[test]
fn css_class_accepts_string_and_list() {
    let owned = String::from("muted app");
    let from_string = elm_magic::ui! { <Text class={owned}>"hi"</Text> };
    assert_eq!(from_string.class().to_vec(), ["muted", "app"]);

    let list = vec!["muted", "card"];
    let from_list = elm_magic::ui! { <Row class={list}></Row> };
    assert_eq!(from_list.class().to_vec(), ["muted", "card"]);

    let array = ["muted", "card"];
    let from_array = elm_magic::ui! { <Row class={array}></Row> };
    assert_eq!(from_array.class().to_vec(), ["muted", "card"]);
}

#[test]
fn css_class_is_empty_when_absent() {
    let el = elm_magic::ui! { <Text>"hi"</Text> };
    assert!(el.class().is_empty());
}

// ── 셀터 5형태 (토큰 / 문자열 / 결합자 / 상태 / 그룹) ─────

elm_magic::css! {
    * { margin: 1; }
    Col > Row { gap: 3; }
    ".a .b" { gap: 2; }
    button:hover { bg: surface; }
    ".x, .y" { gap: 4; }
}

#[test]
fn css_selector_forms_register() {
    let get = |sel: &str, key: &str| {
        elm_magic::style::lookup(sel)
            .unwrap_or_else(|| panic!("{sel} 미등록"))
            .get(key)
    };
    assert_eq!(get("*", "margin").as_deref(), Some("1"), "전체 렉터");
    assert_eq!(
        get("Col>Row", "gap").as_deref(),
        Some("3"),
        "자식은 `>`로 (공백 없이)"
    );
    assert_eq!(
        get(".a .b", "gap").as_deref(),
        Some("2"),
        "문자열 렉터는 공백 보존"
    );
    assert_eq!(
        get("button:hover", "bg").as_deref(),
        Some("surface"),
        "상태"
    );
    assert_eq!(get(".x", "gap").as_deref(), Some("4"), "그룹은 각각 등록");
    assert_eq!(get(".y", "gap").as_deref(), Some("4"));
}
