//! 0.7.3 — 지역/전역 상태의 **수명** 회귀 테스트.
//!
//! `callbacks.rs`와 같은 방식으로, "당연히 이럴 것"과 실제가 갈리는 지점을
//! 실제 라이브러리에서 실행해 고정한다.
//!
//! 1) unmount 뒤 도착한 **지역 상태** 쓰기(`+=`/`=`/`push`)는 조용히 버려진다 —
//!    예전에는 `on_unmount { n += 1 }`이 "slot not initialized"로 패닉했다.
//! 2) unmount된 인스턴스의 **구독**(`on_message`/`->`)은 스스로 끝난다 —
//!    예전에는 죽은 컴포넌트가 전역 상태를 계속 오염시켰고, 재마운트하면
//!    값이 두 번씩 배달됐다.
//! 3) store 키는 **선언 모듈까지** 포함한다 — 서로 다른 모듈의 동명 store가
//!    같은 슬롯을 공유해 타입 불일치로 패닉하던 문제.
//! 4) 렌더 **중** 전역 상태가 바뀌면 그 프레임을 한 번 더 그린다 —
//!    같은 트리의 형제가 서로 다른 값을 보지 않는다.
//!
//! 명세가 모호한 지점은 "현재 의미론" 주석과 함께 고정한다 (6절).

use elm_magic::prelude::*;

// ── 1. unmount 뒤 지역 상태 쓰기 ────────────────────────────

elm_magic::view! {
    fn UnmountAdd(n = 0) {
        on_unmount { n += 1 }
        <Text>"n: {n}"</Text>
    }
}

elm_magic::view! {
    fn UnmountSet(n = 0) {
        on_unmount { n = 5 }
        <Text>"n: {n}"</Text>
    }
}

elm_magic::view! {
    fn UnmountHost(show = true, which = 0) {
        <Col>
            {if show && which == 0 { <UnmountAdd /> } else { <Text>"-"</Text> }}
            {if show && which == 1 { <UnmountSet /> } else { <Text>"-"</Text> }}
            <Button on_click={show = !show}>"toggle"</Button>
        </Col>
    }
}

#[test]
fn local_add_in_on_unmount_is_dropped_instead_of_panicking() {
    // 예전: `n += 1` → 죽은 슬롯에 mutate → "slot not initialized" 패닉.
    let mut app = elm_magic::testing::mount_with::<UnmountHost>(UnmountHostProps {
        which: Some(0),
        ..Default::default()
    });
    app.click("toggle"); // 자식 unmount → on_unmount가 지역 슬롯에 쓴다
    app.assert_text("-");
    app.click("toggle"); // 재마운트 → 상태는 초기값
    app.assert_text("n: 0");
}

#[test]
fn local_set_in_on_unmount_is_dropped() {
    // `n = 5`(set)는 패닉하지는 않았지만 죽은 슬롯을 **되살렸다** — 이제 버린다.
    let mut app = elm_magic::testing::mount_with::<UnmountHost>(UnmountHostProps {
        which: Some(1),
        ..Default::default()
    });
    app.click("toggle");
    app.assert_text("-");
    app.click("toggle");
    app.assert_text("n: 0");
}

// ── 2. unmount된 인스턴스의 구독 ────────────────────────────

fn feed() -> Vec<i32> {
    vec![1, 2, 3]
}

fn ticks() -> Vec<i32> {
    vec![10, 20]
}

#[store]
struct Counters {
    total: i32,
}

elm_magic::view! {
    fn FeedLocal(total = 0) {
        on_message(feed(), v) { total += v }
        <Text>"total: {total}"</Text>
    }
}

elm_magic::view! {
    fn FeedStore() {
        on_message(feed(), v) { counters.total += v }
        <Text>"store: {counters.total}"</Text>
    }
}

elm_magic::view! {
    fn StreamToStore(pct = 0) {
        <Col>
            <Button on_click={ticks() -> pct { counters.total += pct }}>"go"</Button>
            "pct: {pct}"
        </Col>
    }
}

elm_magic::view! {
    fn SubHost(show = true, which = 0) {
        <Col>
            "now: {counters.total}"
            {if show && which == 0 { <FeedLocal /> } else { <Text>"-"</Text> }}
            {if show && which == 1 { <FeedStore /> } else { <Text>"-"</Text> }}
            {if show && which == 2 { <StreamToStore /> } else { <Text>"-"</Text> }}
            <Button on_click={show = !show}>"toggle"</Button>
        </Col>
    }
}

#[test]
fn local_subscription_stops_at_unmount() {
    let mut app = elm_magic::testing::mount_with::<SubHost>(SubHostProps {
        which: Some(0),
        ..Default::default()
    });
    app.pump();
    app.assert_text("total: 1");
    app.click("toggle"); // 구독 대상 unmount
    app.pump(); // 죽은 구독은 아무 일도 하지 않는다 (패닉도 없이)
    app.assert_text("-");
}

#[test]
fn store_is_not_written_by_a_dead_subscription() {
    let mut app = elm_magic::testing::mount_with::<SubHost>(SubHostProps {
        which: Some(1),
        ..Default::default()
    });
    app.pump();
    app.assert_text("store: 1");
    app.click("toggle");
    app.pump();
    // 예전에는 죽은 구독이 계속 전역 상태를 올려 "now: 2"가 됐다.
    app.assert_text("now: 1");
}

#[test]
fn remount_subscribes_exactly_once() {
    let mut app = elm_magic::testing::mount_with::<SubHost>(SubHostProps {
        which: Some(1),
        ..Default::default()
    });
    app.click("toggle"); // unmount (구독 해제)
    app.click("toggle"); // 재마운트 → 새 구독 1개
    app.pump();
    // 예전에는 옛 태스크가 살아 있어 값이 두 번 배달됐다 (now: 2).
    app.assert_text("store: 1");
    app.assert_text("now: 1");
}

#[test]
fn arrow_stream_stops_at_unmount() {
    let mut app = elm_magic::testing::mount_with::<SubHost>(SubHostProps {
        which: Some(2),
        ..Default::default()
    });
    app.click("go"); // 스트림 스폰 (대상 `pct` 슬롯이 수명 가드)
    app.click("toggle"); // 대상 unmount
    app.pump(); // 죽은 스트림 → 본문이 실행되지 않는다
    app.assert_text("now: 0");
}

// ── 3. 동명 store, 다른 모듈 ────────────────────────────────

mod alpha {
    use elm_magic::prelude::*;

    #[store]
    pub struct Pref {
        level: i32,
    }

    elm_magic::view! {
        pub fn AlphaLevel() {
            <Text>"alpha: {pref.level}"</Text>
        }
    }
}

mod beta {
    use elm_magic::prelude::*;

    #[store]
    pub struct Pref {
        level: bool,
    }

    elm_magic::view! {
        pub fn BetaLevel() {
            <Text>"beta: {pref.level}"</Text>
        }
    }
}

use alpha::{AlphaLevel, AlphaLevelProps};
use beta::{BetaLevel, BetaLevelProps};

elm_magic::view! {
    fn TwoPrefs() {
        <Col>
            <AlphaLevel />
            <BetaLevel />
        </Col>
    }
}

#[test]
fn same_named_stores_in_different_modules_are_independent() {
    // 예전에는 키가 `"Pref.level"` 하나뿐이라 먼저 초기화된 i32 슬롯을 bool로
    // 읽어 "store not initialized (type mismatch?)"로 패닉했다.
    let app = elm_magic::testing::mount::<TwoPrefs>();
    app.assert_text("alpha: 0");
    app.assert_text("beta: false");
}

// ── 4. 렌더 중 전역 상태 쓰기 ───────────────────────────────

#[store]
struct Seeded {
    n: i32,
}

elm_magic::view! {
    fn Seeder() {
        on_mount { seeded.n = 5 }
        <Text>"seeder: {seeded.n}"</Text>
    }
}

elm_magic::view! {
    fn SeedReader() {
        <Text>"reader: {seeded.n}"</Text>
    }
}

elm_magic::view! {
    fn SeedOrder() {
        <Col>
            <SeedReader />
            <Seeder />
        </Col>
    }
}

#[test]
fn render_time_store_write_is_visible_to_earlier_siblings() {
    // `on_mount`은 렌더 중에 실행된다 — 예전에는 먼저 그려진 형제가 0을 봤다
    // (같은 트리 안에서 서로 다른 값을 보였다).
    let app = elm_magic::testing::mount::<SeedOrder>();
    app.assert_text("reader: 5");
    app.assert_text("seeder: 5");
}

// ── 5. key={id}로 항목을 따라가는 상태 (대조군) ─────────────

elm_magic::view! {
    fn KeyedItem(id = 0, n = 0) {
        <Row>
            "{id}:{n}"
            <Button on_click={n += 1}>"inc{id}"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn KeyedList(ids: Vec<i32> = vec![]) {
        <Col>
            {ids.map(|id| <Row key={id}><KeyedItem id={id} /></Row>)}
            <Button on_click={ids.reverse()}>"rev"</Button>
        </Col>
    }
}

#[test]
fn keyed_items_keep_state_across_reorder() {
    let mut app = elm_magic::testing::mount_with::<KeyedList>(KeyedListProps {
        ids: Some(vec![1, 2]),
        ..Default::default()
    });
    app.click("inc1");
    app.assert_text("1:1");
    app.click("rev");
    // 키가 같으면 상태가 항목을 따라간다
    app.assert_text("1:1");
    app.assert_text("2:0");
}

// ── 6. 현재 의미론 (바뀌면 테스트가 알려준다) ───────────────

elm_magic::view! {
    fn PropChild(text = String::new()) {
        <Text>"child: {text}"</Text>
    }
}

elm_magic::view! {
    fn PropParent(seed = 1) {
        <Col>
            <PropChild text={format!("s{}", seed)} />
            <Button on_click={seed += 1}>"bump"</Button>
        </Col>
    }
}

#[test]
fn param_is_initial_state_not_a_live_prop() {
    // 사양서 4.1: 매개변수는 **상태 슬롯**이다. 부모가 새 값을 넘겨도 이미
    // 만들어진 자식 인스턴스의 상태는 바뀌지 않는다 (초기값으로만 쓰인다).
    let mut app = elm_magic::testing::mount::<PropParent>();
    app.assert_text("child: s1");
    app.click("bump");
    app.assert_text("child: s1"); // s2가 아니다
}

elm_magic::view! {
    fn PositionItem(id = 0, n = 0) {
        <Row>
            "{id}:{n}"
            <Button on_click={n += 1}>"inc{id}"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn PositionList(ids: Vec<i32> = vec![]) {
        <Col>
            {ids.map(|id| <PositionItem id={id} />)}
            <Button on_click={ids.reverse()}>"rev"</Button>
        </Col>
    }
}

#[test]
fn keyless_list_state_follows_position_not_item() {
    // `key={id}`가 없으면 인스턴스 경로가 형제 순번(`@0`, `@1`)이라 상태가
    // **자리**를 따라간다. 게다가 `id` prop도 초기값(사양서 4.1)이라,
    // 목록을 뒤집어도 화면은 그대로다 — 항목을 따라가려면 5절처럼 `key`를 쓴다.
    let mut app = elm_magic::testing::mount_with::<PositionList>(PositionListProps {
        ids: Some(vec![1, 2]),
        ..Default::default()
    });
    app.click("inc1");
    let before = app.render_tree();
    app.assert_text("1:1");
    app.click("rev");
    assert_eq!(
        app.render_tree(),
        before,
        "키 없는 목록은 상태도 prop도 자리를 따라간다 (현재 의미론)"
    );
}
