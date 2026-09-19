//! 사양서 12 "사용자 계약" 7개를 실행 가능한 형태로 고정한다.
//!
//! 이 파일은 `src/`를 보지 않고 **사양서 · README · 기존 테스트가 쓰는 공개 API만**으로
//! 작성했다. 구현이 아니라 계약을 검사하므로, 계약이 깨지면 여기가 가장 먼저 알려준다.
//!
//! | 계약 (사양서 12) | 테스트 |
//! |---|---|
//! | 같은 상태 → 같은 트리 | `same_state_yields_identical_tree` |
//! | 컴포넌트는 순수 함수 | `render_does_not_mutate_state`, `rendering_again_does_not_rerun_lifecycle` |
//! | 자식 상태는 부모에 안 샘 | `siblings_keep_independent_state` |
//! | 키가 바뀌면 초기화 | `key_change_resets_child_state` |
//! | Cmd는 flush 전까지 실행 안 됨 | `effect_never_runs_without_flush`, `flush_does_not_rerun_effects` |
//! | 뷰는 직렬화 가능 | `every_builtin_tag_serializes` (`--features serde`) |
//! | 상태는 원본을 변형 안 함 | `reading_the_store_does_not_write_it` |

use elm_magic::prelude::*;

// ── 계약 1·2: 같은 상태 → 같은 트리, 렌더는 순수하다 ─────────

elm_magic::view! {
    fn Stable(n = 0, items: Vec<String> = vec![]) {
        <Col>
            <Button on_click={n += 1}>"inc"</Button>
            "n: {n}"
            {items.map(|i| <Row>"{i}"</Row>)}
        </Col>
    }
}

#[test]
fn same_state_yields_identical_tree() {
    let app = elm_magic::mount!(Stable);
    let first = app.render_tree();
    for round in 1..=20 {
        assert_eq!(
            app.render_tree(),
            first,
            "{round}번째 렌더가 첫 렌더와 다르다 — 렌더는 순수 함수여야 한다"
        );
    }
}

#[test]
fn render_does_not_mutate_state() {
    let app = elm_magic::mount!(Stable);
    for _ in 0..10 {
        let _ = app.render_tree();
    }
    app.assert_text("n: 0");
}

elm_magic::view! {
    fn MountOnce(mounts = 0) {
        on_mount { mounts += 1 }
        <Text>"mounts: {mounts}"</Text>
    }
}

#[test]
fn rendering_again_does_not_rerun_lifecycle() {
    let app = elm_magic::mount!(MountOnce);
    app.assert_text("mounts: 1");
    for _ in 0..5 {
        let _ = app.render_tree();
    }
    app.assert_text("mounts: 1");
}

// ── 계약 3: 자식 상태는 부모에 안 샌다 ──────────────────────

elm_magic::view! {
    fn Child(n = 0) {
        <Row>
            "child {n}"
            <Button on_click={n += 1}>"child+{n}"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn Siblings() {
        <Col>
            <Child />
            <Child />
            <Child />
        </Col>
    }
}

#[test]
fn siblings_keep_independent_state() {
    let mut app = elm_magic::mount!(Siblings);
    // 라벨이 겹치면 첫 매치가 클릭된다 (현재 의미론 — tests/edge_cases.rs에서도 고정)
    app.click("child+0");
    app.assert_text("child 1");
    // 나머지 둘은 초기 상태 그대로다
    let text = app.text();
    assert_eq!(text.matches("child 0").count(), 2, "text: {text}");
    assert_eq!(text.matches("child 1").count(), 1, "text: {text}");
}

// ── 계약 4: 키가 바뀌면 초기화 ──────────────────────────────

elm_magic::view! {
    fn ContractKeyed(id = 0, n = 0) {
        <Row>
            "k{id}:{n}"
            <Button on_click={n += 1}>"k{id}+"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn ContractKeyHost(ids: Vec<i32> = vec![]) {
        <Col>
            {ids.map(|id| <Row key={id}><ContractKeyed id={id} /></Row>)}
            <Button on_click={ids.remove(0)}>"drop-first"</Button>
            <Button on_click={ids.push(1)}>"re-add"</Button>
        </Col>
    }
}

#[test]
fn key_change_resets_child_state() {
    let mut app = elm_magic::mount_with::<ContractKeyHost>(ContractKeyHostProps {
        ids: Some(vec![1, 2]),
        ..Default::default()
    });
    app.click("k1+");
    app.assert_text("k1:1");

    // key=1이 사라진다 → 슬롯 폐기 → 다시 나타나면 **새 인스턴스**(초기 상태)
    app.click("drop-first");
    app.assert_hidden("k1:1");
    app.click("re-add");
    app.assert_text("k1:0");
    app.assert_hidden("k1:1");
}

// ── 계약 5: Cmd는 flush 전까지 실행되지 않는다 ──────────────

async fn contract_load(id: i32) -> String {
    format!("loaded-{id}")
}

elm_magic::view! {
    fn Loader(value = String::new()) {
        <Col>
            <Button on_click={value <- contract_load(1)}>"go"</Button>
            "value: {value}"
        </Col>
    }
}

#[test]
fn effect_never_runs_without_flush() {
    let mut app = elm_magic::mount!(Loader);
    app.assert_text("value: ");

    app.click("go"); // 효과를 예약한다
    app.assert_text("value: ");
    app.advance(10_000); // 시간이 흘러도 flush 전에는 실행되지 않는다
    app.assert_text("value: ");

    app.flush();
    app.assert_text("value: loaded-1");
}

#[test]
fn flush_does_not_rerun_effects() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let app = elm_magic::mount!(Loader);
    let mut app = app.mock(contract_load, move |_id: i32| {
        counter.fetch_add(1, Ordering::SeqCst);
        "mocked".to_string()
    });

    app.click("go");
    app.flush();
    app.assert_text("value: mocked");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "예약된 효과는 한 번만 실행된다"
    );

    app.flush();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "빈 큐를 다시 flush해도 효과는 다시 돌지 않는다"
    );
}

// ── 계약 7: 상태는 원본을 변형하지 않는다 (읽기는 쓰기가 아니다) ──

#[store]
struct Ledger {
    n: i32,
}

elm_magic::view! {
    fn LedgerReader() {
        <Text>"n: {ledger.n}"</Text>
    }
}

elm_magic::view! {
    fn LedgerWriter() {
        <Button on_click={ledger.n += 1}>"inc"</Button>
    }
}

elm_magic::view! {
    fn LedgerRoot() {
        <Col>
            <LedgerReader />
            <LedgerWriter />
            <LedgerReader />
        </Col>
    }
}

#[test]
fn reading_the_store_does_not_write_it() {
    let app = elm_magic::mount!(LedgerRoot);
    let key = concat!(module_path!(), "::Ledger.n");
    let before = app.ctx.arena.store_version(key);
    for _ in 0..5 {
        let _ = app.render_tree();
    }
    assert_eq!(
        app.ctx.arena.store_version(key),
        before,
        "읽기만 하는 렌더는 전역 상태 버전을 올리지 않는다"
    );
    app.assert_text("n: 0");
}

#[test]
fn every_reader_in_one_tree_sees_the_same_store_value() {
    let mut app = elm_magic::mount!(LedgerRoot);
    app.click("inc");
    let text = app.text();
    assert_eq!(
        text.matches("n: 1").count(),
        2,
        "같은 트리의 형제가 서로 다른 값을 보면 안 된다: {text}"
    );
}

// ── 계약 6: 뷰는 직렬화 가능 (사양서 8.3, 선택 기능) ─────────

elm_magic::view! {
    fn ContractGallery(
        text = String::new(),
        checked = false,
        open = false,
        tab = 0,
    ) {
        <Col>
            <Row>"row"</Row>
            <Text class="t">"text"</Text>
            <Strong>"strong"</Strong>
            <Button>"button"</Button>
            <Input value={text.clone()} on_change={text = _} />
            <TextArea value={text.clone()} on_change={text = _} />
            <Check checked={checked} on_change={checked = _}>"check"</Check>
            <Tab active={tab == 0} on_click={tab = 0}>"tab"</Tab>
            <Th on_click={tab = 1}>"th"</Th>
            <Td>"td"</Td>
            <Banner kind="error">"banner"</Banner>
            <Spinner />
            <Divider />
            <Progress value={0.5} />
            <Modal on_close={open = false}><Text>"modal"</Text></Modal>
            <Raw>|_ui: &mut ()| {}</Raw>
        </Col>
    }
}

#[cfg(feature = "serde")]
#[test]
fn every_builtin_tag_serializes() {
    let app = elm_magic::mount!(ContractGallery);
    let json = serde_json::to_string(app.element()).expect("Element: Serialize");
    for kind in [
        "Col", "Row", "Text", "Strong", "Button", "Input", "TextArea", "Check", "Tab", "Th", "Td",
        "Banner", "Spinner", "Divider", "Progress", "Modal", "Raw",
    ] {
        assert!(
            json.contains(&format!("\"{kind}\"")),
            "{kind}가 스냅샷에 없다: {json}"
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn serialized_view_is_deterministic_and_skips_handlers() {
    let json = |()| {
        let app = elm_magic::mount!(ContractGallery);
        serde_json::to_string(app.element()).expect("Element: Serialize")
    };
    assert_eq!(json(()), json(()), "같은 상태 → 같은 JSON");
    let one = json(());
    for handler in ["on_click", "on_change", "on_close", "widget"] {
        assert!(!one.contains(handler), "{handler}가 직렬화에 남았다: {one}");
    }
}
