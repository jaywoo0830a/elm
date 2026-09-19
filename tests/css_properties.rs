//! v0.8 — `css!` 확장 속성 (코어 계약).
//!
//! 어댑터(egui/gpui)가 아니라 **코어**에서 검증한다 — `css!` 등록 텍스트와
//! `style::resolve`가 만든 `ResolvedStyle` 값까지. 어댑터는 이 값을 옮기기만 한다.

use elm_magic::style::{self, Align, BorderStyle, Direction, Edges, Overflow, Palette};

elm_magic::css! {
    .ext_box {
        flex-direction: column; flex-grow: 1; flex-shrink: 0.5;
        align-self: center; overflow: hidden;
        aspect-ratio: 1.5; z-index: -1;
        padding: 8; padding-top: 1; padding-right: 2; padding-bottom: 3; padding-left: 4;
        margin: 8; margin-top: 5; margin-left: 6;
        border-width: 2; border-style: dashed;
        border-top-width: 1; border-right-width: 2; border-bottom-width: 3; border-left-width: 4;
        white-space: nowrap; text-overflow: ellipsis; max-lines: 3;
        rotate: 45; scale: 1.25; pointer-events: none;
    }
    .ext_row {
        flex-direction: row; flex-grow: 2; align-self: end; overflow: scroll; z-index: 10;
        white-space: normal; text-overflow: clip; pointer-events: auto;
        border-style: solid;
    }
}

fn resolved(classes: &[&str]) -> style::ResolvedStyle {
    let classes: Vec<String> = classes.iter().map(|s| s.to_string()).collect();
    style::resolve(&classes, "", &Palette::dark())
}

fn get(selector: &str, key: &str) -> Option<String> {
    style::lookup_class(selector)
        .unwrap_or_else(|| panic!("{selector} 미등록"))
        .get(key)
}

#[test]
fn css_ext_props_register_text() {
    assert_eq!(get(".ext_box", "flex-direction").as_deref(), Some("column"));
    assert_eq!(get(".ext_box", "flex-grow").as_deref(), Some("1"));
    assert_eq!(get(".ext_box", "flex-shrink").as_deref(), Some("0.5"));
    assert_eq!(get(".ext_box", "align-self").as_deref(), Some("center"));
    assert_eq!(get(".ext_box", "overflow").as_deref(), Some("hidden"));
    assert_eq!(get(".ext_box", "aspect-ratio").as_deref(), Some("1.5"));
    assert_eq!(get(".ext_box", "z-index").as_deref(), Some("-1"));
    assert_eq!(get(".ext_box", "padding-top").as_deref(), Some("1"));
    assert_eq!(get(".ext_box", "padding-right").as_deref(), Some("2"));
    assert_eq!(get(".ext_box", "padding-bottom").as_deref(), Some("3"));
    assert_eq!(get(".ext_box", "padding-left").as_deref(), Some("4"));
    assert_eq!(get(".ext_box", "margin-top").as_deref(), Some("5"));
    assert_eq!(get(".ext_box", "margin-left").as_deref(), Some("6"));
    assert_eq!(get(".ext_box", "border-style").as_deref(), Some("dashed"));
    assert_eq!(get(".ext_box", "border-top-width").as_deref(), Some("1"));
    assert_eq!(get(".ext_box", "border-left-width").as_deref(), Some("4"));
    assert_eq!(get(".ext_box", "white-space").as_deref(), Some("nowrap"));
    assert_eq!(
        get(".ext_box", "text-overflow").as_deref(),
        Some("ellipsis")
    );
    assert_eq!(get(".ext_box", "max-lines").as_deref(), Some("3"));
    assert_eq!(get(".ext_box", "rotate").as_deref(), Some("45"));
    assert_eq!(get(".ext_box", "scale").as_deref(), Some("1.25"));
    assert_eq!(get(".ext_box", "pointer-events").as_deref(), Some("none"));
}

#[test]
fn css_ext_props_resolve_to_core_values() {
    let s = resolved(&["ext_box"]);
    assert_eq!(s.direction, Some(Direction::Column));
    assert_eq!(s.flex_grow, Some(1.0));
    assert_eq!(s.flex_shrink, Some(0.5));
    assert_eq!(s.align_self, Some(Align::Center));
    assert_eq!(s.overflow, Some(Overflow::Hidden));
    assert_eq!(s.aspect_ratio, Some(1.5));
    assert_eq!(s.z_index, Some(-1));
    // 숏핸드와 개별 값이 함께 산다
    assert_eq!(s.padding, Some(Edges::splat(8.0)));
    assert_eq!(s.padding_top, Some(1.0));
    assert_eq!(s.padding_left, Some(4.0));
    assert_eq!(s.margin, Some(Edges::splat(8.0)));
    assert_eq!(s.margin_top, Some(5.0));
    assert_eq!(s.margin_left, Some(6.0));
    assert_eq!(s.border_width, Some(2.0));
    assert_eq!(s.border_style, Some(BorderStyle::Dashed));
    assert_eq!(s.border_top_width, Some(1.0));
    assert_eq!(s.border_left_width, Some(4.0));
    assert_eq!(s.nowrap, Some(true));
    assert_eq!(s.ellipsis, Some(true));
    assert_eq!(s.max_lines, Some(3));
    assert_eq!(s.rotate, Some(45.0));
    assert_eq!(s.scale, Some(1.25));
    assert_eq!(s.pointer_events, Some(false));
}

#[test]
fn css_ext_bool_false_variants() {
    let s = resolved(&["ext_row"]);
    assert_eq!(s.direction, Some(Direction::Row));
    assert_eq!(s.flex_grow, Some(2.0));
    assert_eq!(s.align_self, Some(Align::End));
    assert_eq!(s.overflow, Some(Overflow::Scroll));
    assert_eq!(s.z_index, Some(10));
    assert_eq!(s.border_style, Some(BorderStyle::Solid));
    assert_eq!(s.nowrap, Some(false));
    assert_eq!(s.ellipsis, Some(false));
    assert_eq!(s.pointer_events, Some(true));
}

#[test]
fn white_space_inherits_but_layout_does_not() {
    let parent = resolved(&["ext_box"]);
    let mut child = style::ResolvedStyle::default();
    child.inherit_from(&parent);
    assert_eq!(child.nowrap, Some(true), "white-space는 상속된다");
    assert_eq!(child.direction, None, "flex-direction은 상속되지 않는다");
    assert_eq!(child.rotate, None, "rotate는 상속되지 않는다");
    assert_eq!(child.overflow, None, "overflow는 상속되지 않는다");
}
