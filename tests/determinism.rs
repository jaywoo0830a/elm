//! 결정성 — "같은 입력 → 같은 출력"을 흔들 수 있는 요인을 하나씩 제거한다.
//!
//! 스타일 레지스트리(`HashMap`)의 순회 순서, 클래스 나열 순서, 렌더 반복,
//! 인스턴스 개수, 목(mock) 등록 범위가 결과를 바꾸지 않음을 고정한다.
//! 전역 레지스트리를 건드리는 테스트(`style::reset_for_tests`)는 여기 두지 않는다 —
//! `tests/init_styles.rs`가 단독 프로세스에서 담당한다.

// ── 스타일 해석의 결정성 (사양서 6.1) ───────────────────────

elm_magic::css! {
    .det_a { gap: 4; padding: 2; bg: surface; }
    .det_b { gap: 8; }
}

fn resolve_classes(classes: &[&str]) -> elm_magic::style::ResolvedStyle {
    let classes: Vec<String> = classes.iter().map(|s| s.to_string()).collect();
    elm_magic::style::resolve(&classes, "", &elm_magic::style::Palette::dark())
}

#[test]
fn style_resolution_is_independent_of_class_order() {
    let ab = resolve_classes(&["det_a", "det_b"]);
    let ba = resolve_classes(&["det_b", "det_a"]);

    // CSS와 같다: `class` 나열 순서가 아니라 **규칙 선언 순서**가 이긴다.
    assert_eq!(ab.gap, Some(8.0), ".det_b가 나중에 선언됐다");
    assert_eq!(
        ba.gap,
        Some(8.0),
        "class 나열 순서는 캐스케이드에 영향이 없다"
    );
    assert_eq!(ab.padding, ba.padding, "선언하지 않은 속성도 같다");
    assert_eq!(ab.bg, ba.bg);
    assert_eq!(ab.color, ba.color);
}

#[test]
fn style_resolution_is_stable_across_repeated_calls() {
    let first = resolve_classes(&["det_a", "det_b"]);
    for round in 1..=100 {
        let again = resolve_classes(&["det_a", "det_b"]);
        assert_eq!(again.gap, first.gap, "{round}번째 호출에서 gap이 달라졌다");
        assert_eq!(again.padding, first.padding);
        assert_eq!(again.bg, first.bg);
    }
}

#[test]
fn style_lookup_is_stable_across_repeated_calls() {
    let first = elm_magic::style::lookup_class("det_a").expect(".det_a");
    for _ in 0..100 {
        assert_eq!(
            elm_magic::style::lookup_class("det_a").expect(".det_a"),
            first,
            "같은 클래스의 선언 목록은 항상 같다"
        );
    }
}

// ── 렌더의 결정성 ───────────────────────────────────────────

elm_magic::view! {
    fn DetCounter(n = 0) {
        <Col>
            "n: {n}"
            <Button on_click={n += 1}>"inc"</Button>
            <Row class="det_a">"styled"</Row>
        </Col>
    }
}

#[test]
fn render_tree_is_stable_across_repeated_reads() {
    let app = elm_magic::mount!(DetCounter);
    let first = app.render_tree();
    assert_eq!(app.render_tree(), first);
    assert_eq!(app.render_tree(), first);
}

#[test]
fn refresh_does_not_change_the_tree() {
    let mut app = elm_magic::mount!(DetCounter);
    let first = app.render_tree();
    app.refresh();
    assert_eq!(app.render_tree(), first, "상태가 그대로면 트리도 그대로다");
}

#[test]
fn independent_instances_render_identically() {
    let a = elm_magic::mount!(DetCounter);
    let b = elm_magic::mount!(DetCounter);
    let c = elm_magic::mount!(DetCounter);
    assert_eq!(a.render_tree(), b.render_tree());
    assert_eq!(b.render_tree(), c.render_tree());
}

#[test]
fn a11y_tree_is_deterministic() {
    let app = elm_magic::mount!(DetCounter);
    let first = app.a11y_tree();
    assert_eq!(app.a11y_tree(), first);
    assert_eq!(app.roles(), app.roles(), "역할 목록도 매번 같다");
}

#[test]
fn click_result_does_not_depend_on_the_instance() {
    let mut a = elm_magic::mount!(DetCounter);
    let mut b = elm_magic::mount!(DetCounter);
    for _ in 0..3 {
        a.click("inc");
        b.click("inc");
    }
    assert_eq!(a.render_tree(), b.render_tree(), "같은 입력 → 같은 트리");
}

// ── 목(mock) 등록 범위 — 현재 의미론 ────────────────────────

async fn det_load(id: i32) -> String {
    format!("real-{id}")
}

elm_magic::view! {
    fn DetLoader(value = String::new()) {
        <Col>
            <Button on_click={value <- det_load(1)}>"go"</Button>
            "value: {value}"
        </Col>
    }
}

#[test]
fn mock_registry_is_process_global_current_semantics() {
    // 현재 의미론(0.7.4 실측): 목 등록은 **앱 인스턴스가 아니라 프로세스 전역**이다.
    // 그래서 한 테스트에서 목을 걸면 **같은 테스트 바이너리의 다른 앱도** 목을 본다.
    // 테스트를 쓸 때 주의: 같은 효과 함수를 두 테스트에서 다르게 목하면 서로 간섭한다
    // (테스트 바이너리는 파일 단위로 프로세스가 분리되므로 파일 경계는 안전하다).
    let app_a = elm_magic::mount!(DetLoader);
    let mut app_a = app_a.mock(det_load, |_id: i32| "mocked".to_string());
    let mut app_b = elm_magic::mount!(DetLoader);

    app_a.click("go");
    app_a.flush();
    app_b.click("go");
    app_b.flush();

    app_a.assert_text("value: mocked");
    assert!(
        app_b.text().contains("value: mocked"),
        "현재 의미론: 목은 전역이라 새 인스턴스도 목을 본다 (바뀌면 이 테스트가 알려준다)"
    );
}
