// v0.6 — 스타일 해석 (사양서 6.1 · 6.2 · 6.3)
//
// egui 없이 **순수 함수만** 검증한다: `resolve()`가 태그/클래스를 합쳐
// `ResolvedStyle`을 만드는 계약 전체가 여기에 있다.

use elm_magic::style::{self, Color, Edges, Palette, State, Token};

elm_magic::css! {
    .resolve_card { gap: 8; padding: 16; bg: surface; radius: 8; }
    .resolve_loud { gap: 32; }
    .resolve_muted { color: text_dim; }
    .resolve_done { color: text_dim; text-decoration: line-through; }
    .resolve_title { font-size: 24; weight: bold; }
    .resolve_plain { weight: normal; font-size: 12; }
    .resolve_under { text-decoration: underline; }
    .resolve_box { margin: 0 -8; padding: 8 16 4 2; }
    .resolve_px { gap: 12px; font-size: 18px; }
    button { radius: 6; bg: primary; }
}

// 렉터 · 상속 · 상태 (v0.6+)
elm_magic::css! {
    // 태그 형태: 인접한 단어는 후손, `>`는 직계 자식, 붙은 클래스는 복합
    .sel_root Button { color: text_dim; font-size: 12; }
    .sel_root > Text { padding: 4; }
    // 문자열 형태: 공백까지 그대로 (진짜 CSS)
    ".sel_root .sel_leaf" { color: info; }
    ".sel_root .sel_leaf:hover" { color: error; }
    ".sel_root .sel_leaf:disabled" { color: warn; }
    .sel_root { color: success; font-size: 20; padding: 8; }
    .sel_leaf { font-size: 12; }
    .inh_root { color: success; font-size: 20; padding: 8; }
    .inh_leaf { font-size: 12; }
    .merge_a, .merge_b { gap: 3; }
}

// 속성 36종 전부 (한 규칙에 선언 → `pairs()`로 한 번에 확인)
elm_magic::css! {
    .everything {
        gap: 6; row-gap: 4; column-gap: 8;
        padding: 8 16; margin: 0 4;
        width: 240; height: fill; min-width: 100; max-height: 400;
        align: center; justify: end; wrap: true;
        display: none; visibility: hidden;
        bg: surface; fill: primary;
        border-width: 2; border-color: border; radius: 10;
        shadow: 4 8; shadow-color: shadow; opacity: 0.5;
        color: text_dim; font-size: 14; line-height: 20; letter-spacing: 0.5;
        weight: bold; font-style: italic; font-family: monospace;
        text-decoration: line-through; text-align: center; text-transform: uppercase;
        truncate: true; cursor: pointer;
    }
}

fn style_of(classes: &[&str]) -> style::ResolvedStyle {
    let classes: Vec<String> = classes.iter().map(|s| s.to_string()).collect();
    style::resolve(&classes, "", &Palette::dark())
}

#[test]
fn resolve_is_empty_without_selectors() {
    assert!(style_of(&[]).is_empty());
    assert!(style_of(&["never_registered"]).is_empty());
}

#[test]
fn resolve_reads_class_declarations() {
    let s = style_of(&["resolve_card"]);
    assert_eq!(s.gap, Some(8.0));
    assert_eq!(s.padding, Some(Edges::splat(16.0)));
    assert_eq!(s.bg, Some(Palette::dark().get(Token::Surface)));
    assert_eq!(s.radius, Some(8.0));
    assert_eq!(s.color, None, "선언하지 않은 속성은 비어 있다");
}

#[test]
fn resolve_cascade_follows_css() {
    // CSS와 같다: `class` 나열 순서가 아니라 **규칙 선언 순서**가 이긴다.
    // (`.resolve_card`가 먼저, `.resolve_loud`가 나중에 선언다)
    let a = style_of(&["resolve_card", "resolve_loud"]);
    assert_eq!(a.gap, Some(32.0), "나중 규칙이 이긴다");
    assert_eq!(a.padding, Some(Edges::splat(16.0)), "안 은 속성은 남는다");

    let b = style_of(&["resolve_loud", "resolve_card"]);
    assert_eq!(
        b.gap,
        Some(32.0),
        "class 나열 순서는 캐스케이드에 영향 없다"
    );
}

#[test]
fn resolve_tag_spec_is_overridden_by_class() {
    // `.resolve_loud`는 gap만 선언 → 태그 `button`의 radius/bg는 살아 있다
    let classes = vec!["resolve_loud".to_string()];
    let s = style::resolve(&classes, "button", &Palette::dark());
    assert_eq!(s.gap, Some(32.0), "클래스가 선언한 속성");
    assert_eq!(s.radius, Some(6.0), "태그만 선언한 속성은 살아 있다");
    assert_eq!(s.bg, Some(Palette::dark().get(Token::Primary)));

    // 겹치면 클래스가 이긴다
    let over = vec!["resolve_card".to_string()];
    let s2 = style::resolve(&over, "button", &Palette::dark());
    assert_eq!(
        s2.bg,
        Some(Palette::dark().get(Token::Surface)),
        "같은 속성을 선언하면 클래스가 태그를 덮는다"
    );
}

#[test]
fn resolve_tag_only() {
    let s = style::resolve(&[], "button", &Palette::dark());
    assert_eq!(s.radius, Some(6.0));
    assert_eq!(s.bg, Some(Palette::dark().get(Token::Primary)));
}

#[test]
fn resolve_merges_multiple_classes() {
    let s = style_of(&["resolve_muted", "resolve_title"]);
    assert_eq!(s.color, Some(Palette::dark().get(Token::TextDim)));
    assert_eq!(s.font_size, Some(24.0));
    assert_eq!(s.bold, Some(true));
}

#[test]
fn resolve_text_decoration_and_weight() {
    let done = style_of(&["resolve_done"]);
    assert_eq!(done.strike, Some(true));
    assert_eq!(done.underline, Some(false), "숏핸드는 나머지를 끈다");

    let under = style_of(&["resolve_under"]);
    assert_eq!(under.underline, Some(true));
    assert_eq!(under.strike, Some(false));

    assert_eq!(
        style_of(&["resolve_plain"]).bold,
        Some(false),
        "weight: normal"
    );
}

#[test]
fn resolve_edges_shorthands() {
    let s = style_of(&["resolve_box"]);
    assert_eq!(
        s.padding,
        Some(Edges::new(8.0, 16.0, 4.0, 2.0)),
        "4값 숏핸드"
    );
    assert_eq!(
        s.margin,
        Some(Edges {
            top: 0.0,
            right: -8.0,
            bottom: 0.0,
            left: -8.0
        }),
        "음수 값"
    );
}

#[test]
fn resolve_accepts_px_suffix() {
    // 값에 단위를 붙여도 된다 (`16` ≡ `16px`)
    let s = style_of(&["resolve_px"]);
    assert_eq!(s.gap, Some(12.0));
    assert_eq!(s.font_size, Some(18.0));
    assert_eq!(
        elm_magic::style::lookup_class("resolve_px")
            .unwrap()
            .get("gap"),
        Some("12".to_string()),
        "선언 텍스트에는 단위가 남지 않는다"
    );
}

#[test]
fn palette_light_differs_from_dark() {
    let dark = Palette::dark();
    let light = Palette::light();
    assert_ne!(dark.get(Token::Surface), light.get(Token::Surface));
    assert_ne!(dark.get(Token::Text), light.get(Token::Text));
    assert_eq!(dark.get(Token::Error), Color::rgb(239, 68, 68));
    assert_eq!(light.get(Token::Error), Color::rgb(220, 38, 38));
}

#[test]
fn palette_custom_theme_flows_into_resolution() {
    // 사양서 6.3 — 테마는 팔레트다
    let theme = Palette::dark().with(Token::Primary, Color::rgb(1, 2, 3));
    let tag = style::resolve(&[], "button", &theme);
    assert_eq!(
        tag.bg,
        Some(Color::rgb(1, 2, 3)),
        "토큰이 테마 색으로 확정된다"
    );

    let classes = vec!["resolve_card".to_string()];
    let s = style::resolve(&classes, "", &theme);
    assert_eq!(s.bg, Some(theme.get(Token::Surface)));
}

#[test]
fn token_names_round_trip() {
    for (name, token) in Token::ALL {
        assert_eq!(Token::from_name(name), Some(*token));
        assert_eq!(token.name(), *name);
    }
    assert_eq!(Token::from_name("nope"), None);
}

#[test]
fn element_resolved_style_uses_tag_and_class() {
    let el = elm_magic::ui! { <Button class="resolve_loud">"go"</Button> };
    assert_eq!(el.tag(), "button");
    let s = el.resolved_style(&Palette::dark());
    assert_eq!(
        s.bg,
        Some(Palette::dark().get(Token::Primary)),
        "태그 `button`의 bg"
    );
    assert_eq!(s.radius, Some(6.0), "태그 `button`의 radius");
    assert_eq!(s.gap, Some(32.0), "클래스 `.resolve_loud`의 gap");
}

#[test]
fn element_tags_are_lowercase_names() {
    let el = elm_magic::ui! {
        <Col>
            <Text>"a"</Text>
            <Strong>"b"</Strong>
            <Button>"c"</Button>
            <Input />
        </Col>
    };
    assert_eq!(el.tag(), "col");
    let children = el.children().expect("Col has children");
    let tags: Vec<&str> = children.iter().map(|c| c.tag()).collect();
    assert_eq!(tags, ["text", "strong", "button", "input"]);
}

#[test]
fn fragment_has_no_tag() {
    // 본문이 요소 목록이면 `Fragment`로 감싼다 (`into_element`)
    let el = elm_magic::into_element(
        elm_magic::ui! { {if true { <Text>"a"</Text> } else { <Text>"b"</Text> }} },
    );
    assert_eq!(el.tag(), "", "Fragment는 태그 셀렉터와 안 맞는다");
}

#[test]
fn into_classes_normalizes_inputs() {
    use style::IntoClasses;
    assert_eq!("a b".into_classes(), ["a", "b"]);
    assert_eq!("  a ".into_classes(), ["a"]);
    assert_eq!("".into_classes(), Vec::<String>::new());
    assert_eq!(String::from("x y").into_classes(), ["x", "y"]);
    assert_eq!(vec!["a b", "c"].into_classes(), ["a", "b", "c"]);
    assert_eq!(["a", "b"].into_classes(), ["a", "b"]);
    assert_eq!(Vec::<String>::new().into_classes(), Vec::<String>::new());
}

#[test]
fn selector_parsing_specificity_and_matching() {
    use style::{Comb, Node, Selector, State};

    let sp = |s: &str| Selector::parse(s).unwrap().specificity();
    assert_eq!(sp("*"), (0, 0));
    assert_eq!(sp("button"), (0, 1));
    assert_eq!(sp(".card"), (1, 0));
    assert_eq!(sp(".a.b.c"), (3, 0));
    assert_eq!(sp(".card Button"), (1, 1));
    assert_eq!(sp("button:hover"), (1, 1));
    assert_eq!(sp(".a .b > c:hover"), (3, 1));

    let sel = Selector::parse(".a.b > button:hover").unwrap();
    assert_eq!(sel.parts.len(), 2);
    assert_eq!(sel.combs, [Comb::Child]);
    let ab = vec!["a".to_string(), "b".to_string()];
    let none: Vec<String> = vec![];
    let parent = Node::new("col", &ab);
    let child = Node::new("button", &none);
    let hover = State::new(true, false, false, false);
    assert!(sel.matches(&[child, parent], hover));
    assert!(!sel.matches(&[child, parent], State::NONE), ":hover 필요");
    assert!(!sel.matches(&[parent, child], hover), "자신이 먼저");
    assert!(!sel.matches(&[child], hover), "조상 필요");

    // 깨진 셀터는 None
    for bad in ["> a", "a >", ":nope", "..a", "a b >", "", "  "] {
        assert!(
            Selector::parse(bad).is_none(),
            "`{bad}`는 파싱 실패해야 한다"
        );
    }
}

#[test]
fn descendant_and_child_selectors_use_the_tree() {
    let tree = elm_magic::ui! {
        <Col class="sel_root">
            <Text class="sel_leaf">"직계"</Text>
            <Row><Text class="sel_leaf">"손자"</Text></Row>
            <Button>"go"</Button>
        </Col>
    };
    let kids = tree.children().expect("children");
    let direct = &kids[0];
    let row = &kids[1];
    let deep = &row.children().expect("row children")[0];
    let button = &kids[2];
    let p = Palette::dark();

    // 직계는 `.sel_root > Text`가 맞는다
    let s = direct.resolved_style_in(&[&tree], State::NONE, None, &p);
    assert_eq!(s.padding, Some(Edges::splat(4.0)), "`.sel_root > Text`");

    // 손자는 `>`가 안 맞고 후손은 맞는다
    let deep_s = deep.resolved_style_in(&[row, &tree], State::NONE, None, &p);
    assert_eq!(deep_s.padding, None, "`>`는 직계만 — Row가 끼면 안 맞는다");
    assert_eq!(
        deep_s.color,
        Some(p.get(Token::Info)),
        "후손은 레벨을 건너다"
    );
    assert_eq!(deep_s.font_size, Some(12.0), "자기 규칙이 상속을 이긴다");

    // 토큰 형태 후손(`.sel_root Button`) + 태그 규칙
    let bs = button.resolved_style_in(&[&tree], State::NONE, None, &p);
    assert_eq!(bs.color, Some(p.get(Token::TextDim)));
    assert_eq!(bs.font_size, Some(12.0));
    assert_eq!(bs.radius, Some(6.0), "`button` 태그 규칙(다른 블록)도 함께");

    // 조상이 없으면 후손/자식은 안 맞는다
    assert_eq!(
        direct.resolved_style_in(&[], State::NONE, None, &p).padding,
        None
    );
}

#[test]
fn state_selectors_gate_on_runtime_state() {
    let tree = elm_magic::ui! { <Col class="sel_root"><Text class="sel_leaf">"x"</Text></Col> };
    let leaf = &tree.children().expect("children")[0];
    let p = Palette::dark();

    let plain = leaf.resolved_style_in(&[&tree], State::NONE, None, &p);
    assert_eq!(plain.color, Some(p.get(Token::Info)));

    let hovered = leaf.resolved_style_in(&[&tree], State::new(true, false, false, false), None, &p);
    assert_eq!(hovered.color, Some(p.get(Token::Error)), ":hover가 이긴다");

    let off = leaf.resolved_style_in(&[&tree], State::new(false, false, false, true), None, &p);
    assert_eq!(off.color, Some(p.get(Token::Warn)), ":disabled");
}

#[test]
fn inheritable_properties_flow_down() {
    let tree = elm_magic::ui! {
        <Col class="inh_root"><Row class="inh_leaf"><Text class="inh_leaf">"x"</Text></Row></Col>
    };
    let row = &tree.children().expect("children")[0];
    let text = &row.children().expect("children")[0];
    let p = Palette::dark();

    let root = tree.resolved_style_in(&[], State::NONE, None, &p);
    assert_eq!(root.color, Some(p.get(Token::Success)));
    assert_eq!(root.padding, Some(Edges::splat(8.0)));

    let row_style = row.resolved_style_in(&[&tree], State::NONE, Some(&root), &p);
    assert_eq!(row_style.color, Some(p.get(Token::Success)), "color는 상속");
    assert_eq!(row_style.font_size, Some(12.0), "자기 규칙이 상속을 이다");
    assert_eq!(row_style.padding, None, "padding은 상속되지 않는다");

    let text_style = text.resolved_style_in(&[row, &tree], State::NONE, Some(&row_style), &p);
    assert_eq!(text_style.color, Some(p.get(Token::Success)), "2단계 상속");
    assert_eq!(text_style.font_size, Some(12.0));
    assert_eq!(text_style.padding, None);
}

#[test]
fn extended_properties_are_parsed() {
    let props = style::lookup_class("everything").expect(".everything");
    let got: Vec<(String, String)> = props.pairs();
    let want = [
        ("gap", "6"),
        ("row-gap", "4"),
        ("column-gap", "8"),
        ("padding", "8 16"),
        ("margin", "0 4"),
        ("width", "240"),
        ("height", "fill"),
        ("min-width", "100"),
        ("max-height", "400"),
        ("align", "center"),
        ("justify", "end"),
        ("wrap", "true"),
        ("display", "none"),
        ("visibility", "hidden"),
        ("bg", "surface"),
        ("fill", "primary"),
        ("border-width", "2"),
        ("border-color", "border"),
        ("radius", "10"),
        ("shadow", "0 4 8"),
        ("shadow-color", "shadow"),
        ("opacity", "0.5"),
        ("color", "text_dim"),
        ("font-size", "14"),
        ("line-height", "20"),
        ("letter-spacing", "0.5"),
        ("weight", "bold"),
        ("font-style", "italic"),
        ("font-family", "monospace"),
        ("text-decoration", "line-through"),
        ("text-align", "center"),
        ("text-transform", "uppercase"),
        ("truncate", "true"),
        ("cursor", "pointer"),
    ];
    let want: Vec<(String, String)> = want
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn extended_properties_resolve_to_typed_values() {
    let s = style_of(&["everything"]);
    assert_eq!(s.width, Some(style::Len::Px(240.0)));
    assert_eq!(s.height, Some(style::Len::Fill));
    assert_eq!(s.min_width, Some(100.0));
    assert_eq!(s.max_height, Some(400.0));
    assert_eq!(s.align, Some(style::Align::Center));
    assert_eq!(s.justify, Some(style::Align::End));
    assert_eq!(s.wrap, Some(true));
    assert!(s.is_display_none(), "display: none");
    assert_eq!(s.hidden, Some(true), "visibility: hidden");
    assert_eq!(s.border_width, Some(2.0));
    assert_eq!(
        s.shadow,
        Some(style::Shadow {
            dx: 0.0,
            dy: 4.0,
            blur: 8.0,
            spread: 0.0
        })
    );
    assert_eq!(s.opacity, Some(0.5));
    assert_eq!(s.transform, Some(style::Transform::Upper));
    assert_eq!(s.cursor, Some(style::Cursor::Pointer));
    assert_eq!(s.line_height, Some(20.0));
    assert_eq!(s.letter_spacing, Some(0.5));
    assert_eq!(s.italic, Some(true));
    assert_eq!(s.mono, Some(true));
    assert_eq!(s.text_align, Some(style::Align::Center));
    assert_eq!(s.truncate, Some(true));
    assert_eq!(s.border_color, Some(Palette::dark().get(Token::Border)));
    assert_eq!(s.shadow_color, Some(Palette::dark().get(Token::Shadow)));
    assert_eq!(s.row_gap(), Some(4.0), "row-gap이 gap을 이긴다");
    assert_eq!(s.column_gap(), Some(8.0));
}

#[test]
fn selector_group_registers_each_selector() {
    for name in ["merge_a", "merge_b"] {
        let props = style::lookup_class(name).unwrap_or_else(|| panic!(".{name}"));
        assert_eq!(props.get("gap").as_deref(), Some("3"));
        assert_eq!(props.selector(), format!(".{name}"));
    }
}

#[test]
fn text_transform_and_into_classes_extras() {
    use style::{IntoClasses, Transform};
    assert_eq!(Transform::Upper.apply("hi there"), "HI THERE");
    assert_eq!(Transform::Lower.apply("HI"), "hi");
    assert_eq!(Transform::Capitalize.apply("hi there"), "Hi There");
    assert_eq!(Transform::None.apply("hi"), "hi");

    let owned = String::from("a b");
    assert_eq!((&owned).into_classes(), ["a", "b"], "&T");
    assert_eq!(Some("a b").into_classes(), ["a", "b"], "Option");
    assert_eq!(Option::<&str>::None.into_classes(), Vec::<String>::new());
}

#[test]
fn spec_none_is_default_and_declares_nothing() {
    use style::StyleSpec;
    assert_eq!(StyleSpec::NONE, StyleSpec::default());
    let mut resolved = style::ResolvedStyle::default();
    resolved.apply(&StyleSpec::NONE, &Palette::dark());
    assert!(resolved.is_empty());
}
