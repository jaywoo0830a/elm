//! 경계값·강건성 — 사양서에 명시가 없는 "가장자리"를 실행해 고정한다.
//!
//! 빈/거대 리스트, 깊은 중첩, 유니코드, 중복 라벨, 시간 리터럴의 상한, 정수 오버플로,
//! 클래스 정규화. 각 테스트는 **현재 의미론**을 주석으로 남겨, 나중에 바뀌면 알려주게 한다.

use elm_magic::prelude::*;
use std::panic::catch_unwind;

// ── 리스트 크기 ─────────────────────────────────────────────

elm_magic::view! {
    fn EdgeList(items: Vec<i32> = vec![]) {
        <Col>
            {items.map(|i| <Row>"row-{i}"</Row>)}
        </Col>
    }
}

#[test]
fn empty_list_renders_nothing() {
    let app = elm_magic::mount!(EdgeList);
    assert!(!app.text().contains("row-"), "text: {:?}", app.text());
    assert!(app.exists(&Selector::tag("col")));
}

#[test]
fn single_item_list_renders_once() {
    let app = elm_magic::mount_with::<EdgeList>(EdgeListProps {
        items: Some(vec![7]),
        ..Default::default()
    });
    app.assert_text("row-7");
    assert_eq!(app.text().matches("row-").count(), 1);
}

#[test]
fn thousand_item_list_renders_all_items() {
    let app = elm_magic::mount_with::<EdgeList>(EdgeListProps {
        items: Some((0..1000).collect()),
        ..Default::default()
    });
    let text = app.text();
    assert!(text.contains("row-0"), "첫 항목");
    assert!(text.contains("row-999"), "마지막 항목");
    assert_eq!(text.matches("row-").count(), 1000, "항목이 빠지지 않는다");
}

// ── 깊은 중첩 (자기 참조 컴포넌트) ──────────────────────────

elm_magic::view! {
    fn EdgeDeep(depth = 0) {
        <Col>
            {if depth > 0 { <EdgeDeep depth={depth - 1} /> } else { <Text>"bottom"</Text> }}
        </Col>
    }
}

#[test]
fn hundred_level_nesting_does_not_overflow() {
    let app = elm_magic::mount_with::<EdgeDeep>(EdgeDeepProps {
        depth: Some(100),
        ..Default::default()
    });
    app.assert_text("bottom");
}

// ── keyed 항목이 많아도 상태가 서로 새지 않는다 ─────────────

elm_magic::view! {
    fn EdgeItem(id = 0, n = 0) {
        <Row>
            "item {id}: {n}"
            <Button on_click={n += 1}>"i{id}+"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn EdgeMany(ids: Vec<i32> = vec![]) {
        <Col>
            {ids.map(|id| <Row key={id}><EdgeItem id={id} /></Row>)}
        </Col>
    }
}

#[test]
fn many_keyed_items_keep_separate_state() {
    let mut app = elm_magic::mount_with::<EdgeMany>(EdgeManyProps {
        ids: Some((0..200).collect()),
        ..Default::default()
    });
    app.click("i150+");
    app.assert_text("item 150: 1");
    app.assert_text("item 149: 0");
    app.assert_text("item 151: 0");
    app.assert_text("item 0: 0");
}

// ── 유니코드 텍스트 ────────────────────────────────────────

elm_magic::view! {
    fn EdgeInput(value = String::new()) {
        <Col>
            <Input value={value.clone()} on_change={value = _} />
            "v: {value}"
        </Col>
    }
}

#[test]
fn korean_and_emoji_round_trip() {
    let mut app = elm_magic::mount!(EdgeInput);
    app.type_("안녕하세요 🎉");
    app.assert_text("v: 안녕하세요 🎉");
}

#[test]
fn combining_marks_and_zwj_sequences_round_trip() {
    let mut app = elm_magic::mount!(EdgeInput);
    app.type_("e\u{301}"); // 결합 악센트 (e + U+0301)
    app.assert_text("v: e\u{301}");
    app.type_("👨‍👩‍👧‍👦"); // ZWJ 가족 이모지
    app.assert_text("v: 👨‍👩‍👧‍👦");
}

#[test]
fn text_matching_is_substring_based() {
    // 현재 의미론: `assert_text`/`assert_hidden`은 **부분 문자열** 매치다.
    let mut app = elm_magic::mount!(EdgeInput);
    app.type_("안녕하세요");
    app.assert_text("안녕");
    app.assert_hidden("안녕하세요!");
}

#[test]
fn ten_kilobyte_value_round_trips() {
    let long = "x".repeat(10_000);
    let mut app = elm_magic::mount!(EdgeInput);
    app.type_(&long);
    assert!(
        app.text().len() >= 10_000,
        "긴 문자열도 잘리지 않는다: {}",
        app.text().len()
    );
}

elm_magic::view! {
    fn EdgeArea(text = String::new()) {
        <Col>
            <TextArea value={text.clone()} on_change={text = _} />
            "lines: {text.lines().count()}"
        </Col>
    }
}

#[test]
fn textarea_accepts_multiline_input() {
    let mut app = elm_magic::mount!(EdgeArea);
    app.type_into("textarea", "a\nb\nc");
    app.assert_text("lines: 3");
}

// ── 중복 라벨 ──────────────────────────────────────────────

elm_magic::view! {
    fn EdgeDup(n = 0) {
        <Col>
            "n: {n}"
            <Button on_click={n += 1}>"same"</Button>
            <Button on_click={n += 10}>"same"</Button>
        </Col>
    }
}

#[test]
fn duplicate_labels_click_the_first_match() {
    // 현재 의미론: 같은 라벨이 여러 개면 **트리 순서상 첫 번째**가 클릭된다.
    let mut app = elm_magic::mount!(EdgeDup);
    app.click("same");
    app.assert_text("n: 1");
    app.assert_hidden("n: 10");
}

// ── 시간 리터럴의 경계 ─────────────────────────────────────

elm_magic::view! {
    fn EdgeClock(now = 0) {
        on_tick(1ms) { now += 1 }
        <Text>"now: {now}"</Text>
    }
}

#[test]
fn advance_zero_is_a_noop() {
    let mut app = elm_magic::mount!(EdgeClock);
    app.advance(0);
    app.assert_text("now: 0");
    app.advance(1);
    app.assert_text("now: 1");
}

#[test]
fn advance_does_not_catch_up_missed_intervals() {
    // 현재 의미론: `advance(10_000)`은 1ms 틱을 10_000번 실행하지 않고 **한 번만** 발화한다.
    let mut app = elm_magic::mount!(EdgeClock);
    app.advance(10_000);
    app.assert_text("now: 1");
}

elm_magic::view! {
    fn EdgeHugeTick(now = 0) {
        on_tick(4294967296ms) { now += 1 }
        <Text>"now: {now}"</Text>
    }
}

elm_magic::view! {
    fn EdgeMaxTick(now = 0) {
        on_tick(18446744073709551615ms) { now += 1 }
        <Text>"now: {now}"</Text>
    }
}

#[test]
fn huge_tick_intervals_are_accepted_and_do_not_fire_early() {
    // 2^32ms(≈49일) — 오버플로 없이 예약된다
    let mut app = elm_magic::mount!(EdgeHugeTick);
    app.advance(1);
    app.assert_text("now: 0");

    // u64::MAX ms — 상한값도 panic 없이 받아들인다
    let mut max = elm_magic::mount!(EdgeMaxTick);
    max.advance(1);
    max.assert_text("now: 0");
}

// 참고: `on_tick(0ms)`는 주기가 0이라 `advance()`가 끝나지 않을 수 있어 자동 테스트로
// 두지 않는다 (수동 확인용). 사양서에 최소 주기 규칙이 없다.

// ── 파싱 불가한 시간 리터럴 — 결함 후보 ─────────────────────
//
// `u64`를 넘는 리터럴이 **컴파일 에러도, 런타임 에러도 아니고 0ms**가 된다.
// `on_change … after`에서는 지연이 사라지고, 같은 경로가 `on_tick`이면 0ms 주기가 되어
// `advance()`가 끝나지 않을 수 있다. 그래서 여기서는 안전한 `on_change`로 관측한다.

elm_magic::view! {
    fn EdgeTooBigDuration(query = String::new(), fired = String::new()) {
        // 2^64 — u64에 들어가지 않는다
        on_change(query) after 18446744073709551616ms { fired = query.clone() }
        <Col>
            <Input value={query.clone()} on_change={query = _} />
            "fired: {fired}"
        </Col>
    }
}

elm_magic::view! {
    fn EdgeFitDuration(query = String::new(), fired = String::new()) {
        // 2^32 — u64에 들어간다 (대조군)
        on_change(query) after 4294967296ms { fired = query.clone() }
        <Col>
            <Input value={query.clone()} on_change={query = _} />
            "fired: {fired}"
        </Col>
    }
}

#[test]
fn unparseable_duration_literal_silently_becomes_zero() {
    // 현재 의미론(실측) — **결함 후보**: 컴파일 에러가 아니라 0ms로 떨어져,
    // 클럭을 진행하지 않아도 즉시 발화한다 (= 지연이 조용히 사라진다).
    // 사양서 6.1/README의 "잘못된 값은 컴파일 에러" 계약과 어긋나므로,
    // 리터럴 검증이 붙으면 이 테스트를 뒤집는다.
    let mut app = elm_magic::mount!(EdgeTooBigDuration);
    app.type_("x");
    app.assert_text("fired: x");

    // 대조군: u64 안에 들어가는 값은 지연이 살아 있다
    let mut control = elm_magic::mount!(EdgeFitDuration);
    control.type_("x");
    control.assert_hidden("fired: x");
}

// ── 정수 오버플로 (빌드 프로파일에 따라 의미가 다르다) ──────

elm_magic::view! {
    fn EdgeOverflow(n: i32 = i32::MAX) {
        <Col>
            "n: {n}"
            <Button on_click={n += 1}>"inc"</Button>
        </Col>
    }
}

#[cfg(debug_assertions)]
#[test]
fn integer_overflow_panics_in_debug_builds() {
    // `cargo test`(디버그): 오버플로는 panic이다. `cargo test --release`는 래핑한다.
    let err = catch_unwind(|| {
        let mut app = elm_magic::mount!(EdgeOverflow);
        app.click("inc");
    });
    assert!(
        err.is_err(),
        "디버그 빌드에서는 오버플로가 panic이어야 한다"
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn integer_overflow_wraps_in_release_builds() {
    let mut app = elm_magic::mount!(EdgeOverflow);
    app.click("inc");
    app.assert_text(&format!("n: {}", i32::MIN));
}

// ── class 속성 정규화 ──────────────────────────────────────

#[test]
fn class_attribute_normalizes_whitespace() {
    let cases: [(&str, usize); 5] = [("", 0), ("  ", 0), (" a ", 1), ("a  b", 2), ("a b", 2)];
    for (raw, expected) in cases {
        let el = elm_magic::ui! { <Text class={raw.to_string()}>"x"</Text> };
        assert_eq!(el.class().len(), expected, "class={raw:?}");
    }
}

// ── 문서와 실제가 다른 지점 (현재 의미론) ───────────────────

elm_magic::css! {
    h1 { gap: 1; }
    .edge_live { gap: 2; }
}

#[test]
fn unknown_tag_selector_is_silently_accepted() {
    // README · 사양서 6.1은 "잘못된 셀렉터는 컴파일 에러"라고 약속하지만, 현재는
    // 어휘에 없는 **태그 셀렉터**(`h1`)가 오류 없이 등록된다. 어휘에 없는 태그는
    // 어떤 요소도 갖지 않으므로 매치될 수 없는 **죽은 규칙**이 된다.
    // (속성 · 값 · 팔레트 토큰은 컴파일 에러다 — `tests/compile_fail/ui/` 참고.)
    // 검증이 붙으면 이 테스트를 뒤집는다.
    let spec = elm_magic::style::lookup_tag("h1").expect("현재는 그대로 등록된다");
    assert_eq!(spec.get("gap").as_deref(), Some("1"));

    // 죽은 규칙이 다른 태그로 새지 않는다
    let button = elm_magic::style::resolve(&[], "button", &elm_magic::style::Palette::dark());
    assert_eq!(button.gap, None, "h1 규칙은 button에 적용되지 않는다");

    // 같은 블록에서 등록한 정상 클래스는 그대로 동작한다
    let el = elm_magic::ui! { <Col class="edge_live">"x"</Col> };
    let resolved = el.resolved_style(&elm_magic::style::Palette::dark());
    assert_eq!(resolved.gap, Some(2.0));
}
