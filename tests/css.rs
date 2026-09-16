// v0.2 — css! (사양서 6.1)

elm_magic::css! {
    .app { gap: 16; padding: 24; }
    .card { gap: 8; padding: 16; bg: surface; radius: 8; }
    .muted { color: text_dim; }
    button { bg: primary; color: on_primary; padding: 8 16; radius: 6; }
}

#[test]
fn css_registers_classes() {
    let card = elm_magic::style::lookup(".card").expect(".card registered");
    assert_eq!(card.get("gap"), Some("8"));
    assert_eq!(card.get("padding"), Some("16"));
    assert_eq!(card.get("bg"), Some("surface"));
    assert_eq!(card.get("radius"), Some("8"));
    assert_eq!(card.get("color"), None, "card has no color");
}

#[test]
fn css_registers_tag_selectors() {
    let button = elm_magic::style::lookup("button").expect("button registered");
    assert_eq!(button.get("bg"), Some("primary"));
    assert_eq!(button.get("padding"), Some("8 16"));
}

#[test]
fn css_unknown_class_is_none() {
    assert!(elm_magic::style::lookup(".nonexistent").is_none());
}
