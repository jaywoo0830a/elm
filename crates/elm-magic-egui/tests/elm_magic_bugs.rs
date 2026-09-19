//! elm-magic **버그 회귀 테스트 3건** — freedf-gui UI 재설계 중 실측으로 찾은 것.
//!
//! 0.7.4에서 이 3건은 **버그**였고(재현 단언이 통과했다), 0.7.5에서 고쳤다.
//! 이제 이 파일은 **고쳐진 동작**을 잠근다 — 다시 깨지면 여기서 먼저 알려준다.
//!
//! | # | 0.7.4 증상 | 0.7.5 수정 | 근본 원인 |
//! |---|---|---|---|
//! | 1 | `wrap: true`가 무시된다(행이 줄바꿈하지 않음) | 넘치는 자식이 다음 줄로 내려간다 | `layout_of`가 `wrap`을 읽지 않았다 |
//! | 2 | 콘텐츠 크기 자식의 `justify: end`가 무효다 | 부모 폭만큼 늘어나 오른쪽에 붙는다 | `justify` 컨테이너에 주축 크기를 주지 않았다 |
//! | 3 | `width: fill`이 뒤 형제 자리를 비우지 않는다(형제가 창을 넘으면 조상 `max_rect`까지 팽창) | 뒤 형제의 몫을 **먼저 예약**하고 남는 폭만 먹는다 | "그 시점의 남은 폭"을 그대로 썼다 |
//!
//! 이 파일은 freedf-gui와 무관하다: 재현에 필요한 CSS·컴포넌트가 전부 여기 안에
//! 있고, elm-magic 어댑터를 헤드리스 egui(800×600)로 직접 구동한다.
//!
//! 참고: 어댑터는 그리기 전에 egui 기본 스타일을 **리셋**한다(0.7.5) — 여기서는
//! 그 리셋 자체를 검사하지 않는다(egui 버전이 바뀌면 깨질 수 있으므로). 대신
//! `css!`로 선언한 폭·간격만으로 레이아웃 계약을 검사한다.

use elm_magic::style::Palette;

// ── 재현용 CSS (freedf-gui 스타일과 무관) ────────────────────────────────
elm_magic::css! {
    // 케이스 컨테이너 — 폭을 **300px로 고정**해 창 폭과 분리한다.
    .case { width: 300; gap: 4; }
    // `wrap: true` 재현용(같은 컨테이너에 wrap만 추가).
    .wrapcase { wrap: true; }
    // 케이스 3 — 창(800)보다 살짝 좁은 컨테이너.
    .fillcase { width: 780; gap: 4; }
    // 폭 고정 버튼 — 텍스트/폰트에 따라 흔들리지 않게.
    .wide { width: 200; min-height: 20; }
    .mid { width: 100; min-height: 20; }
    // `justify: end`만 있는 **콘텐츠 크기** 자식 Row.
    .jrow { justify: end; gap: 4; }
    // 남은 폭을 전부 먹는 스페이서(자식 없음).
    .fill { width: fill; }
}

// ── 재현용 컴포넌트 ──────────────────────────────────────────────────────
elm_magic::view! {
    /// 케이스 1 — 폭 300 안에 200px 버튼 3개(합 608px) + `wrap: true`.
    pub fn WrapCase() {
        <Row class="case wrapcase">
            <Button class="wide">"A"</Button>
            <Button class="wide">"B"</Button>
            <Button class="wide">"C"</Button>
        </Row>
    }

    /// 케이스 2 — 폭 300 안의 콘텐츠 크기 자식 Row에 `justify: end`.
    pub fn JustifyCase() {
        <Row class="case">
            <Row class="jrow">
                <Button class="mid">"J"</Button>
            </Row>
        </Row>
    }

    /// 케이스 3 — 창(800)보다 좁은 780 컨테이너 안에 `width: fill` 스페이서 + 100px 버튼.
    pub fn FillCase() {
        <Row class="fillcase">
            <Col class="fill" />
            <Button class="mid">"F"</Button>
        </Row>
    }
}

/// 한 프레임 렌더 결과 — 버튼 사각형과 루트 ui의 `max_rect` 폭.
struct Rendered {
    buttons: Vec<egui::Rect>,
    root_max_width: f32,
}

/// 헤드리스로 컴포넌트 하나를 그리고 버튼 사각형을 모은다.
fn render<C: elm_magic::Component>(
    elm: &mut elm_magic::Ctx,
    props: &C::Props,
    window: egui::Vec2,
) -> Rendered {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, window)),
        ..Default::default()
    };
    let palette = Palette::dark();
    let mut buttons = Vec::new();
    let mut root_max_width = 0.0;
    let mut out = ctx.run_ui(input, |ui| {
        let tree = elm_magic::frame::<C>(elm, props);
        let pass = elm_magic_egui::render_with_palette(ui, &tree, &mut elm.arena, &palette);
        buttons.extend(pass.buttons.iter().map(|(_, r)| r.rect));
        root_max_width = ui.max_rect().width();
    });
    out.textures_delta.clear();
    Rendered {
        buttons,
        root_max_width,
    }
}

const WINDOW: egui::Vec2 = egui::vec2(800.0, 600.0);
/// 케이스 1·2 컨테이너 폭, 케이스 3 컨테이너 폭 (CSS와 같은 값).
const CASE_W: f32 = 300.0;
const FILL_W: f32 = 780.0;
const FILL_GAP: f32 = 4.0;
const MID_W: f32 = 100.0;

/// **버그 1 (0.7.5에서 수정)** — `wrap: true`가 줄바꿈한다.
#[test]
fn wrap_true_breaks_the_row() {
    let mut elm = elm_magic::Ctx::default();
    let r = render::<WrapCase>(&mut elm, &WrapCaseProps::default(), WINDOW);
    eprintln!("[1] wrap:true — buttons = {:?}", r.buttons);
    assert_eq!(r.buttons.len(), 3, "버튼 3개가 그려져야 한다");
    // 200px 버튼 + gap 4 → 300px 안에는 한 줄에 하나씩만 들어간다.
    let (b0, b1, b2) = (r.buttons[0], r.buttons[1], r.buttons[2]);
    assert!(
        b1.min.y > b0.min.y + 1.0 && b2.min.y > b1.min.y + 1.0,
        "wrap이 동작하면 버튼이 차례로 다음 줄로 내려가야 한다: {:?}",
        r.buttons
    );
    // 모든 버튼이 컨테이너 안에 남는다 (0.7.4에는 3번이 x≈408까지 나갔다).
    for b in &r.buttons {
        assert!(
            b.max.x <= CASE_W + 1.0,
            "버튼이 컨테이너({CASE_W})를 넘어갔다: {:?}",
            b
        );
    }
}

/// **버그 2 (0.7.5에서 수정)** — 콘텐츠 크기 자식의 `justify: end`가 오른쪽에 붙는다.
#[test]
fn justify_end_on_content_sized_child_fills_the_parent() {
    let mut elm = elm_magic::Ctx::default();
    let r = render::<JustifyCase>(&mut elm, &JustifyCaseProps::default(), WINDOW);
    eprintln!("[2] justify:end — buttons = {:?}", r.buttons);
    assert_eq!(r.buttons.len(), 1);
    let b = r.buttons[0];
    assert!(
        b.width() <= MID_W + 1.0,
        "버튼은 선언한 폭({MID_W})을 지켜야 한다(측정 {})",
        b.width()
    );
    // 부모 300 − 버튼 100 = 200 지점에 붙는다.
    assert!(
        b.min.x >= CASE_W - MID_W - 1.0,
        "justify: end가 동작하면 버튼이 오른쪽에 붙어야 한다(측정 x={})",
        b.min.x
    );
    assert!(
        b.max.x <= CASE_W + 1.0,
        "버튼이 컨테이너({CASE_W})를 넘어갔다(측정 {})",
        b.max.x
    );
}

/// **버그 3 (0.7.5에서 수정)** — `width: fill`이 뒤 형제의 자리를 예약한다.
///
/// 0.7.4 실측: 스페이서가 "그 시점의 남은 폭"(780)을 전부 먹어 버튼이 컨테이너 밖
/// (x≈784)으로 밀리고, 그 오버플로가 창(800)을 넘어 조상 `max_rect`를 팽창시켰다.
/// (freedf-gui 실측: 캔버스 폭 1754 > 창 1100, 루트 rect 1241 — 오버플로 + fill 조합.)
#[test]
fn width_fill_reserves_room_for_following_siblings() {
    let mut elm = elm_magic::Ctx::default();
    let r = render::<FillCase>(&mut elm, &FillCaseProps::default(), WINDOW);
    eprintln!(
        "[3] width:fill — buttons = {:?}, root max width = {}",
        r.buttons, r.root_max_width
    );
    assert_eq!(r.buttons.len(), 1);
    let b = r.buttons[0];
    // 스페이서는 780 − 100(버튼) − 4(gap) = 676만 차지한다.
    assert!(
        b.min.x >= FILL_W - MID_W - FILL_GAP - 1.0,
        "스페이서가 뒤 형제의 몫까지 먹어 버튼이 밀렸다(측정 x={})",
        b.min.x
    );
    assert!(
        b.max.x <= FILL_W + 1.0,
        "버튼이 컨테이너({FILL_W}) 밖으로 나갔다(측정 {})",
        b.max.x
    );
    // 오버플로가 없으므로 조상도 창 밖으로 팽창하지 않는다.
    assert!(
        r.root_max_width <= WINDOW.x + 1.0,
        "루트 max_rect가 창({}) 밖으로 팽창했다(측정 {})",
        WINDOW.x,
        r.root_max_width
    );
}
