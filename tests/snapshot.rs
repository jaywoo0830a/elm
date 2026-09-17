#![cfg(feature = "serde")]
//! 사양서 8.3 / 부록 — "뷰는 직렬화 가능"(`Element: Serialize`) 스냅샷 계약.
//!
//! `--features serde`로 실행한다:
//!
//! ```sh
//! cargo test -p elm-magic --features serde
//! ```
//!
//! 핸들러(`Rc<dyn Fn>`)와 `<Raw>` 클로저는 직렬화 대상이 아니므로
//! `#[serde(skip)]`으로 빠지고, **구조·텍스트·클래스만** JSON으로 남는다.
//! `render_tree()`가 사람이 읽는 텍스트 덤프라면, 이쪽은 기계가 비교하는
//! 스냅샷(insta 등)용 계약이다.

elm_magic::css! {
    .snap_card { gap: 8; padding: 16; }
}

elm_magic::view! {
    fn Card(title = String::new(), done = false) {
        <Row class="snap_card">
            <Strong>"{title}"</Strong>
            <Check checked={done} on_change={done = _}>"done"</Check>
            <Button on_click={done = !done}>"toggle"</Button>
            <Raw>|_ui: &mut ()| {}</Raw>
        </Row>
    }
}

fn card_json() -> String {
    let app = elm_magic::mount_with::<Card>(CardProps {
        title: Some("hi".to_string()),
        done: Some(true),
        ..Default::default()
    });
    serde_json::to_string(app.element()).expect("Element must be Serialize")
}

#[test]
fn element_serializes_structure_text_and_class() {
    let json = card_json();
    assert!(json.contains("\"Row\""), "{json}");
    assert!(json.contains("\"snap_card\""), "{json}");
    assert!(json.contains("\"Strong\""), "{json}");
    assert!(json.contains("\"hi\""), "{json}");
    assert!(json.contains("\"Check\""), "{json}");
    assert!(json.contains("\"checked\":true"), "{json}");
    assert!(json.contains("\"toggle\""), "{json}");
}

#[test]
fn serialization_skips_handlers_and_raw_widgets() {
    let json = card_json();
    // 클로저는 스냅샷에 들어가지 않는다
    assert!(!json.contains("on_click"), "{json}");
    assert!(!json.contains("on_change"), "{json}");
    assert!(!json.contains("widget"), "{json}");
}

#[test]
fn serialized_snapshot_is_deterministic() {
    // 같은 상태 → 같은 트리 → 같은 JSON (스냅샷 계약의 전제)
    assert_eq!(card_json(), card_json());
}

#[test]
fn style_props_serialize() {
    let style = elm_magic::style::lookup(".snap_card").expect(".snap_card registered");
    let json = serde_json::to_string(&style).expect("StyleProps must be Serialize");
    assert!(json.contains("\"gap\""), "{json}");
    assert!(json.contains("\"8\""), "{json}");
}
