//! 인스턴스 격리 — 같은 프로세스에 **동시에 살아 있는** 여러 앱이 서로 새지 않는지,
//! 그리고 마운트/언마운트를 반복해도 아레나 자원이 쌓이지 않는지 고정한다.
//!
//! 전역 상태(`#[store]`)만 의도적으로 공유된다. 지역 상태·구독·목은 인스턴스 경계를
//! 넘지 않아야 한다. 테스트 간 간섭을 피하려고 **테스트마다 전용 store 구조체**를 쓴다
//! (store는 전역이고 테스트는 한 프로세스에서 병렬로 돈다).

use elm_magic::prelude::*;

// ── 지역 상태는 인스턴스마다 독립 ───────────────────────────

elm_magic::view! {
    fn IsoCounter(n = 0) {
        <Row>
            "n: {n}"
            <Button on_click={n += 1}>"n+1"</Button>
        </Row>
    }
}

#[test]
fn two_live_instances_do_not_share_local_state() {
    let mut a = elm_magic::mount!(IsoCounter);
    let mut b = elm_magic::mount!(IsoCounter);

    a.click("n+1");
    a.assert_text("n: 1");
    b.assert_text("n: 0");

    b.click("n+1");
    b.click("n+1");
    b.assert_text("n: 2");
    a.assert_text("n: 1");
}

elm_magic::view! {
    fn IsoBoxed(show = true) {
        <Col>
            {if show { <IsoCounter /> } else { <Text>"hidden"</Text> }}
            <Button on_click={show = !show}>"toggle"</Button>
        </Col>
    }
}

#[test]
fn unmount_in_one_instance_does_not_touch_the_other() {
    let mut a = elm_magic::mount!(IsoBoxed);
    let mut b = elm_magic::mount!(IsoBoxed);

    a.click("n+1");
    a.assert_text("n: 1");

    b.click("toggle");
    b.assert_text("hidden");

    a.assert_text("n: 1");
    a.assert_hidden("hidden");
}

// ── keyed 슬롯은 반복 마운트/언마운트에도 쌓이지 않는다 ─────

elm_magic::view! {
    fn IsoItem(id = 0, n = 0) {
        <Row>
            "item {id}: {n}"
            <Button on_click={n += 1}>"i{id}+"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn IsoList(ids: Vec<i32> = vec![], show = true) {
        <Col>
            {if show { <Col>{ids.map(|id| <Row key={id}><IsoItem id={id} /></Row>)}</Col> } else { <Text>"off"</Text> }}
            <Button on_click={show = !show}>"toggle"</Button>
        </Col>
    }
}

#[test]
fn keyed_slots_do_not_accumulate_across_cycles() {
    let mut app = elm_magic::mount_with::<IsoList>(IsoListProps {
        ids: Some(vec![1, 2, 3]),
        ..Default::default()
    });
    let shown = app.keyed_slot_count();
    assert!(shown > 0, "keyed 슬롯이 있어야 한다 (실측 {shown})");

    app.click("toggle");
    let hidden = app.keyed_slot_count();
    assert!(
        hidden < shown,
        "언마운트하면 keyed 슬롯이 줄어든다: {shown} → {hidden}"
    );

    for cycle in 1..=20 {
        app.click("toggle");
        assert_eq!(
            app.keyed_slot_count(),
            shown,
            "{cycle}번째 사이클: 다시 보일 때 슬롯 수가 원래대로여야 한다 (누수 없음)"
        );
        app.click("toggle");
        assert_eq!(
            app.keyed_slot_count(),
            hidden,
            "{cycle}번째 사이클: 숨길 때도 원래대로여야 한다"
        );
    }
}

// ── 전역 상태(`#[store]`)의 실제 범위 — 현재 의미론 ─────────

#[store]
struct Registry {
    n: i32,
}

elm_magic::view! {
    fn RegistryWriter() {
        <Button on_click={registry.n += 1}>"inc"</Button>
    }
}

elm_magic::view! {
    fn RegistryReader() {
        <Text>"shared: {registry.n}"</Text>
    }
}

elm_magic::view! {
    fn RegistryRoot() {
        <Col>
            <RegistryReader />
            <RegistryWriter />
        </Col>
    }
}

#[test]
fn store_is_shared_by_every_component_in_one_instance() {
    let mut app = elm_magic::mount!(RegistryRoot);
    app.assert_text("shared: 0");
    app.click("inc");
    app.click("inc");
    app.assert_text("shared: 2");
}

#[test]
fn a_new_instance_starts_from_the_store_initial_value() {
    // 현재 의미론(0.7.4 실측): store 슬롯은 **아레나 단위**다 — 한 인스턴스 안에서는
    // 모든 컴포넌트가 공유하지만, 새로 만든 인스턴스는 초기값에서 시작한다.
    // 실제 앱은 아레나가 하나뿐이라 체감상 전역 상태이고, 테스트에서는 이 성질이
    // 인스턴스 간 격리를 만들어 준다. (바뀌면 이 테스트가 알려준다.)
    let mut writer = elm_magic::mount!(RegistryWriter);
    writer.click("inc");
    writer.click("inc");

    let reader = elm_magic::mount!(RegistryReader);
    reader.assert_text("shared: 0");
}

// ── 서로 다른 store는 슬롯을 공유하지 않는다 ────────────────

#[store]
struct Alpha {
    n: i32,
}

#[store]
struct Beta {
    n: i32,
}

elm_magic::view! {
    fn AlphaWriter() {
        <Button on_click={alpha.n += 1}>"inc"</Button>
    }
}

elm_magic::view! {
    fn AlphaReader() {
        <Text>"alpha: {alpha.n}"</Text>
    }
}

elm_magic::view! {
    fn BetaReader() {
        <Text>"beta: {beta.n}"</Text>
    }
}

elm_magic::view! {
    fn AlphaBetaRoot() {
        <Col>
            <AlphaReader />
            <AlphaWriter />
            <BetaReader />
        </Col>
    }
}

#[test]
fn distinct_stores_do_not_share_slots() {
    let mut app = elm_magic::mount!(AlphaBetaRoot);
    app.click("inc");
    app.click("inc");
    app.assert_text("alpha: 2");
    app.assert_text("beta: 0");
}

// ── 구독은 인스턴스별로 독립 ────────────────────────────────

fn iso_feed() -> Vec<i32> {
    vec![1]
}

elm_magic::view! {
    fn IsoFeed(total = 0) {
        on_message(iso_feed(), v) { total += v; }
        <Text>"feed total: {total}"</Text>
    }
}

#[test]
fn two_instances_have_independent_subscriptions() {
    let mut a = elm_magic::mount!(IsoFeed);
    let mut b = elm_magic::mount!(IsoFeed);

    a.pump();
    a.assert_text("feed total: 1");
    b.assert_text("feed total: 0");

    b.pump();
    b.assert_text("feed total: 1");
    a.assert_text("feed total: 1");
}
