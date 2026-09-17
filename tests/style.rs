// v0.6 — 스타일 해석 (사양서 6.1 · 6.2 · 6.3)
//
// egui 없이 **순수 함수만** 검증한다: `resolve()`가 태그/클래스를 합쳐
// `ResolvedStyle`을 만드는 계약 전체가 여기에 있다.

use elm_magic::style::{self, Color, Edges, Palette, Token};

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
fn resolve_later_class_wins() {
    let s = style_of(&["resolve_card", "resolve_loud"]);
    assert_eq!(s.gap, Some(32.0), "나열 순서 뒤가 이긴다");
    assert_eq!(s.padding, Some(Edges::splat(16.0)), "안 덮은 속성은 남는다");

    let reversed = style_of(&["resolve_loud", "resolve_card"]);
    assert_eq!(reversed.gap, Some(8.0));
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

    assert_eq!(style_of(&["resolve_plain"]).bold, Some(false), "weight: normal");
}

#[test]
fn resolve_edges_shorthands() {
    let s = style_of(&["resolve_box"]);
    assert_eq!(s.padding, Some(Edges::new(8.0, 16.0, 4.0, 2.0)), "4값 숏핸드");
    assert_eq!(
        s.margin,
        Some(Edges { top: 0.0, right: -8.0, bottom: 0.0, left: -8.0 }),
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
        elm_magic::style::lookup_class("resolve_px").unwrap().get("gap"),
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
    assert_eq!(tag.bg, Some(Color::rgb(1, 2, 3)), "토큰이 테마 색으로 확정된다");

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
    assert_eq!(s.bg, Some(Palette::dark().get(Token::Primary)), "태그 `button`의 bg");
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
fn spec_none_is_default_and_declares_nothing() {
    use style::StyleSpec;
    assert_eq!(StyleSpec::NONE, StyleSpec::default());
    let mut resolved = style::ResolvedStyle::default();
    resolved.apply(&StyleSpec::NONE, &Palette::dark());
    assert!(resolved.is_empty());
}
