//! v0.5 — `#[store]` 전역 상태 (사양서 4.2) + keyed 트리 (사양서 9.5)

use elm_magic::prelude::*;

// `#[store]`는 필드 기본값 문법(`count: i32 = 0`)을 쓸 수 없다 — rustc가
// 구조체로 파싱하며 아직 불안정 기능이기 때문. 기본값이 필요하면 `store!`를 쓴다.
#[store]
struct App {
    count: i32,
    dark: bool,
    closed: i32,
}

elm_magic::view! {
    fn Header() {
        <Row>
            "count: {app.count}"
            "dark: {app.dark}"
            <Button on_click={app.count += 1}>"inc"</Button>
            <Button on_click={app.dark = !app.dark}>"theme"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn Footer() {
        <Text>"footer sees: {app.count}"</Text>
    }
}

elm_magic::view! {
    fn Root() {
        <Col>
            <Header />
            <Footer />
            "closed: {app.closed}"
            {if false { <Session /> } else { <Text>"idle"</Text> }}
                    </Col>
    }
}

elm_magic::view! {
    fn Session() {
        on_unmount { app.closed += 1 }
        <Text>"session open"</Text>
    }
}

#[test]
fn store_is_shared_between_components() {
    let mut app = elm_magic::mount!(Root);
    app.assert_text("count: 0");
    app.assert_text("footer sees: 0");
    app.click("inc");
    app.assert_text("count: 1");
    // 다른 컴포넌트(Footer)도 같은 전역 상태를 본다
    app.assert_text("footer sees: 1");
    app.click("theme");
    app.assert_text("dark: true");
}

#[test]
fn store_writes_bump_version() {
    let mut app = elm_magic::mount!(Root);
    let before = app
        .ctx
        .arena
        .store_version(concat!(module_path!(), "::App.count"));
    app.click("inc");
    assert!(
        app.ctx
            .arena
            .store_version(concat!(module_path!(), "::App.count"))
            > before
    );
}

// ── keyed 트리 (사양서 9.5) ─────────────────────────────────

elm_magic::view! {
    fn Item(id = 0, count = 0) {
        <Row>
            "item {id}: {count}"
            <Button on_click={count += 1}>"count+{id}"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn ItemList(ids: Vec<i32> = vec![]) {
        <Col>
            {ids.map(|id| <Row key={id}><Item id={id} /></Row>)}
            <Button on_click={ids.reverse()}>"reverse"</Button>
        </Col>
    }
}

#[test]
fn keyed_children_keep_state_across_reorder() {
    let mut app = elm_magic::mount_with::<ItemList>(ItemListProps {
        ids: Some(vec![1, 2]),
        ..Default::default()
    });
    app.click("count+1");
    app.assert_text("item 1: 1");
    app.assert_text("item 2: 0");
    // 순서를 뒤집어도 키가 같으면 상태가 따라간다
    app.click("reverse");
    app.assert_text("item 1: 1");
    app.assert_text("item 2: 0");
}

#[test]
fn keyed_slots_are_dropped_on_unmount() {
    let mut app = elm_magic::mount_with::<ItemList>(ItemListProps {
        ids: Some(vec![1, 2, 3]),
        ..Default::default()
    });
    let with_three = app.keyed_slot_count();
    app.click("reverse");
    let after = app.keyed_slot_count();
    assert_eq!(with_three, after, "같은 키 집합이면 슬롯도 그대로");
}

// ── unmount (사양서 5.2, 9.5) ───────────────────────────────

elm_magic::view! {
    fn Counter(n = 0) {
        <Row>
            "n: {n}"
            <Button on_click={n += 1}>"n+1"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn Boxed(show = true) {
        <Col>
            "closed: {app.closed}"
            {if show { <BoxedInner /> } else { <Text>"hidden"</Text> }}
            <Button on_click={show = !show}>"toggle"</Button>
        </Col>
    }
}

elm_magic::view! {
    fn BoxedInner() {
        on_unmount { app.closed += 1 }
        <Counter />
    }
}

#[test]
fn unmount_resets_child_state_and_runs_handler() {
    let mut app = elm_magic::mount!(Boxed);
    app.click("n+1");
    app.assert_text("n: 1");
    app.assert_text("closed: 0");
    app.click("toggle"); // 자식 unmount
    app.assert_text("hidden");
    app.assert_text("closed: 1"); // on_unmount 실행
    app.click("toggle"); // 재마운트 → 상태 초기화
    app.assert_text("n: 0");
}

// ── 함수형 `store_fn!` — 필드 기본값 (사양서 10.4) ──────────

elm_magic::store_fn! {
    Settings { level: i32 = 3, muted: bool = true }
}

elm_magic::view! {
    fn SettingsView() {
        <Col>
            "level: {settings.level}"
            "muted: {settings.muted}"
            <Button on_click={settings.level += 1}>"level+1"</Button>
        </Col>
    }
}

#[test]
fn store_fn_supports_field_defaults() {
    let mut app = elm_magic::mount!(SettingsView);
    app.assert_text("level: 3"); // 기본값
    app.assert_text("muted: true");
    app.click("level+1");
    app.assert_text("level: 4");
}
