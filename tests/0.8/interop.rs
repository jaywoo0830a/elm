//! 0.8.0 호환 — 기존 조건/반복 문법은 그대로 동작하고, 새 태그는 같은 결과를 낸다.
//!
//! 0.8은 **추가 중심**이다 (`0.8-preview.md` §9). `<If>`/`<For>`가 들어와도
//! `{if …}` / `{items.map(…)}`는 계속 쓸 수 있어야 한다.
//!
//! 이 파일은 `cargo test --test v0_8_interop`로 돈다.

// ── 0.7 문법 ────────────────────────────────────────────────
elm_magic::view! {
    fn Legacy(items: Vec<String> = vec![], on = false) {
        <Col>
            {if on { <Text>"on"</Text> } else { <Text>"off"</Text> }}
            {items.iter().map(|i| <Row>"{i}"</Row>)}
        </Col>
    }
}

// ── 0.8 문법 (같은 화면) ────────────────────────────────────
elm_magic::view! {
    fn New(items: Vec<String> = vec![], on = false) {
        <Col>
            <If when={on}>
                <Text>"on"</Text>
            <Else>
                <Text>"off"</Text>
            </Else>
            </If>
            <For each={items} as={i}>
                <Row>"{i}"</Row>
            </For>
        </Col>
    }
}

fn legacy_text(items: Vec<String>, on: bool) -> String {
    elm_magic::mount_with::<Legacy>(LegacyProps {
        items: Some(items),
        on: Some(on),
        ..Default::default()
    })
    .text()
}

fn new_text(items: Vec<String>, on: bool) -> String {
    elm_magic::mount_with::<New>(NewProps {
        items: Some(items),
        on: Some(on),
        ..Default::default()
    })
    .text()
}

#[test]
fn legacy_and_new_syntax_are_equivalent() {
    for on in [false, true] {
        for items in [vec![], vec!["a".to_string(), "b".to_string()]] {
            assert_eq!(
                legacy_text(items.clone(), on),
                new_text(items, on),
                "legacy/new must render the same text (on={on})"
            );
        }
    }
}

#[test]
fn legacy_branches_still_work() {
    let off = elm_magic::mount!(Legacy);
    off.expect_text("off");
    off.assert_hidden("on");

    let on = elm_magic::mount_with::<Legacy>(LegacyProps {
        on: Some(true),
        ..Default::default()
    });
    on.expect_text("on");
    on.assert_hidden("off");
}

// ── 새 태그를 기존 `{if}` 안에 중첩 ─────────────────────────
elm_magic::view! {
    fn Mixed(on = true) {
        <Col>
            {if on {
                <If when={on}>
                    <Text>"nested-new"</Text>
                </If>
            } else {
                <Text>"none"</Text>
            }}
        </Col>
    }
}

#[test]
fn new_tags_nest_inside_legacy_if() {
    let app = elm_magic::mount!(Mixed);
    app.expect_text("nested-new");
    app.assert_hidden("none");

    let app = elm_magic::mount_with::<Mixed>(MixedProps {
        on: Some(false),
        ..Default::default()
    });
    app.expect_text("none");
    app.assert_hidden("nested-new");
}
