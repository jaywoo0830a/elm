//! 버그 리포트(FreeDF 채택 과정) 항목의 회귀 테스트 (테스트 우선 수정).
//!
//! 리포트 항목이 수정되면 리포트 파일은 지우고 **여기 테스트만 남긴다** —
//! 각 테스트는 리포트의 재현 코드를 실제 라이브러리에서 다시 실행한다.
//!
//! 1) `view!`의 `pub fn` / `pub(crate) fn`이 `pub #[derive(..)]`를 생성해
//!    컴파일이 깨지던 문제 (+ `pub(crate)` 의미 보존)
//! 2) `remove(x)` 특수 폼 **뒤에** 오는 문장 구분자가 `,`로 생성되던 문제
//! 3) `{if ..}` 식 내부의 지역 컬렉션 `.iter().map(..)`이 소유 반복으로
//!    재작성되지 않아 E0716(temporary dropped while borrowed)이 나던 문제
//! 4) (0.6.0) BEM 수정자(`.tabs__item--active`)가 `-` 단위로 쪼개져
//!    `.tabs__item - - active`로 조인 → 파싱 실패 → **조용히 미등록**되던 문제

// ── 버그 1: `pub fn` / `pub(crate) fn` ──────────────────────

mod public_component {
    // vis가 `#[derive(Clone)]` **앞에** 찍히면 이 모듈 자체가 컴파일되지 않는다.
    elm_magic::view! {
        pub fn Badge(count = 0) {
            <Row>"count: {count}"</Row>
        }
    }
}

mod crate_component {
    // `pub(crate)` — 그룹 내용이 보존되어야 한다 (`pub`으로 넓어지면 안 됨).
    elm_magic::view! {
        pub(crate) fn Chip(label = String::new()) {
            <Text>"{label}"</Text>
        }
    }
}

#[test]
fn pub_fn_component_is_usable_from_outside_its_module() {
    use public_component::{Badge, BadgeProps};
    let app = elm_magic::testing::mount_with::<Badge>(BadgeProps {
        count: Some(2),
        ..Default::default()
    });
    app.assert_text("count: 2");
}

#[test]
fn pub_crate_fn_component_is_usable_within_the_crate() {
    use crate_component::{Chip, ChipProps};
    let app = elm_magic::testing::mount_with::<Chip>(ChipProps {
        label: Some("hi".to_string()),
        ..Default::default()
    });
    app.assert_text("hi");
}

mod outer {
    pub mod inner {
        // `pub(super)` — 괄호 그룹이 보존되어야 `outer`에서 보인다.
        elm_magic::view! {
            pub(super) fn Tag(text = String::new()) {
                <Text>"{text}"</Text>
            }
        }
    }

    pub fn tag_text(text: &str) -> String {
        use inner::{Tag, TagProps};
        let app = elm_magic::testing::mount_with::<Tag>(TagProps {
            text: Some(text.to_string()),
            ..Default::default()
        });
        app.text()
    }
}

#[test]
fn pub_super_fn_component_keeps_narrow_visibility() {
    // `Tag`는 `outer`까지만 보이므로 여기서는 `outer`의 헬퍼를 통해 접근한다.
    assert_eq!(outer::tag_text("scoped"), "scoped");
}

// ── 버그 2: `remove(x)` 뒤의 문장 구분자 ─────────────────────

elm_magic::view! {
    fn RemoveThenAssign(items: Vec<String> = vec![], last = String::new()) {
        <Col>
            // `remove(x)`가 **첫 문장**이고 뒤에 대입이 이어진다 (리포트의 재현 형태)
            <Button on_click={items.remove(String::from("alpha")), last = String::from("drop")}>"drop-first"</Button>
            // 대조군: `remove(x)`가 마지막 문장 (우회법)
            <Button on_click={last = String::from("set"), items.remove(String::from("beta"))}>"drop-last"</Button>
            {items.map(|t| <Text>"{t}"</Text>)}
            "last: {last}"
        </Col>
    }
}

#[test]
fn remove_special_form_may_be_followed_by_another_statement() {
    let mut app = elm_magic::testing::mount_with::<RemoveThenAssign>(RemoveThenAssignProps {
        items: Some(vec!["alpha".to_string(), "beta".to_string()]),
        last: Some(String::new()),
        ..Default::default()
    });
    app.assert_text("alpha");
    app.assert_text("beta");

    // remove → 대입 순서: 이전에는 `remove` 전개 뒤에 `,`가 찍혀 컴파일 실패
    app.click("drop-first");
    app.assert_hidden("alpha");
    app.assert_text("last: drop");

    // 대조군(remove가 마지막)도 그대로 동작해야 한다
    app.click("drop-last");
    app.assert_hidden("beta");
    app.assert_text("last: set");
}

// ── 버그 3: `{if ..}` 식 내부의 지역 컬렉션 `.iter().map()` ──

elm_magic::view! {
    fn Sections(open = true, status = String::new()) {
        let sections = ["Notes", "PDFs"]; // 렌더 본문의 지역 배열
        <Col>
            {if open {
                {sections.iter().map(|s| <Row on_click={status = format!("{}", s)}>"{s}"</Row>)}
            } else {
                <Text>"none"</Text>
            }}
            "status: {status}"
        </Col>
    }
}

#[test]
fn local_collection_iter_in_conditional_yields_owned_items() {
    // 이전에는 `sections.iter()`가 빌림을 남겨 `'static` 핸들러 캡처에서 E0716
    let mut app = elm_magic::testing::mount::<Sections>();
    app.assert_text("Notes");
    app.assert_text("PDFs");
    app.click("Notes");
    app.assert_text("status: Notes");
    app.click("PDFs");
    app.assert_text("status: PDFs");

    let app = elm_magic::testing::mount_with::<Sections>(SectionsProps {
        open: Some(false),
        ..Default::default()
    });
    app.assert_text("none");
    app.assert_hidden("Notes");
}

elm_magic::view! {
    fn ReuseLocal(total = 0) {
        let values = [1, 2];
        let sum = values.iter().map(|v| v * 2).sum::<i32>();
        <Col>
            {values.iter().map(|v| <Text>"v{v}"</Text>)}
            "sum: {sum}"
            {values.iter().map(|v| <Text>"w{v}"</Text>)}
            <Button on_click={total = sum}>"use"</Button>
        </Col>
    }
}

#[test]
fn local_collection_can_be_iterated_more_than_once() {
    // 슈가가 이동(`into_iter`)이 아니라 클론을 거치므로 지역 변수를 재사용할 수 있다.
    let mut app = elm_magic::testing::mount::<ReuseLocal>();
    app.assert_text("v1");
    app.assert_text("v2");
    app.assert_text("sum: 6");
    app.assert_text("w1");
    app.assert_text("w2");
    app.click("use");
    app.assert_text("sum: 6");
    // total은 이제 sum(6)으로 바뀌지만 화면에는 영향이 없다 — 컴파일·실행 확인용
    app.assert_text("v1");
}

// ── 버그 4 (0.6.0): BEM 수정자(`--`) 클래스가 조용히 미등록 ──────

// 리포트의 최소 재현. `.tabs__item`(요소)은 등록됐지만
// `.tabs__item--active`(수정자)는 `lookup_class`가 `None`을 돌려줬다.
elm_magic::css! {
    .tabs__item { color: text_dim; }
    .tabs__item--active { color: text; }
    .panel__item { gap: 4; }
    .modal__actions--end { justify: end; }
}

#[test]
fn bem_modifier_class_is_registered() {
    let modifier = elm_magic::style::lookup_class("tabs__item--active")
        .expect("`.tabs__item--active`가 등록돼야 한다 (BEM 수정자)");
    assert_eq!(modifier.get("color").as_deref(), Some("text"));
    assert_eq!(
        modifier.selector(),
        ".tabs__item--active",
        "`-`가 별도 단어로 쪼개져 조인되면 안 된다"
    );

    // 대조군: 요소(`__`)는 예전에도 정상 등록됐다
    assert!(elm_magic::style::lookup_class("tabs__item").is_some());
    assert!(elm_magic::style::lookup_class("panel__item").is_some());
    assert!(elm_magic::style::lookup_class("modal__actions--end").is_some());
}

#[test]
fn bem_modifier_class_resolves_and_matches() {
    use elm_magic::style::{resolve, Palette, Token};
    let palette = Palette::dark();
    // 클래스 이름에 `--`가 들어가도 그대로 매칭돼야 한다
    let styled = resolve(&["tabs__item--active".to_string()], "Text", &palette);
    assert_eq!(
        styled.color,
        Some(palette.get(Token::Text)),
        "수정자 규칙이 적용된다"
    );
    let plain = resolve(&["tabs__item".to_string()], "Text", &palette);
    assert_eq!(
        plain.color,
        Some(palette.get(Token::TextDim)),
        "요소 규칙은 그대로"
    );
}

// 같은 원인의 일반형: 하이픈이 들어간 클래스 이름(`.my-class`)도
// `join_selector`가 `-`를 단어로 보면 `.my - class`가 돼 미등록된다.
elm_magic::css! {
    .my-class { gap: 2; }
    .panel__item--active { gap: 3; }
}

#[test]
fn hyphenated_class_name_is_registered() {
    assert!(
        elm_magic::style::lookup_class("my-class").is_some(),
        "`.my-class`가 등록돼야 한다"
    );
    assert!(elm_magic::style::lookup_class("panel__item--active").is_some());
    assert_eq!(
        elm_magic::style::lookup_class("my-class")
            .unwrap()
            .selector(),
        ".my-class"
    );
}
