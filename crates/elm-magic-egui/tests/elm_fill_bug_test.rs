//! elm-magic 0.7.6 **컨테이너/컨테이닝 CSS 시나리오 매트릭스** — 예측 vs 계측.
//!
//! ## 방법
//!
//! 1. 시나리오마다 **예측**을 상댓값(창 대비 비율, 형제 대비 delta)으로 적는다.
//! 2. 헤드리스로 렌더하고 `<Raw>`/`Button` 응답에서 **계측**한다.
//! 3. `|예측 - 계측| > 허용오차`면 **버그**다 — 매트릭스가 그 행을 `BUG!`로 보고한다.
//!
//! 절대 픽셀값은 **판정에 쓰지 않는다**: 여러 픽셀이 얽힌 절대 오프셋 대신
//! "창 - 형제 합 - gap" 같은 **delta**나 "창 대비 %"로만 판정한다. 그래야 창 크기를
//! 바꿔도 같은 결론이 나온다.
//!
//! ## 계측 지점 규약
//!
//! - 컨테이너의 **내부 사각형**: 그 컨테이너의 **첫 자식**으로 `<P tag="..">`를 둔다
//!   (`ui.max_rect()` = 아직 아무것도 안 그린 내부 영역). `<Raw>`는 자리를 차지하지
//!   않지만 gap은 하나 더 붙는다 — 그래서 내부 사각형은 첫 자식으로만 잰다.
//! - 자식 하나의 사각형: 그 자식 **안에** `<P tag="..">`를 둔다.
//! - 폭/높이가 px로 확정된 것은 `<Button>`으로 그려 `pass.buttons` 사각형을 읽는다
//!   (버튼은 `min_size`로 px 폭/높이를 받는다).
//!
//! ## 판정 보조 — CSS가 파싱됐는지 먼저 본다
//!
//! 계측이 예측과 다를 때 원인은 두 갈래다: (a) CSS가 해석되지 않았다,
//! (b) 레이아웃이 잘못 배분했다. `pass.styles`가 **해석된 값**을 주므로
//! `style_of(tag)`로 (a)를 먼저 배제한다 — 남으면 (b) = 버그다.
//!
//! ## 이 파일이 잡은 버그 (매트릭스 `BUG!` 행) — 모두 수정됨
//!
//! | # | 증상 | 실측 delta | 시나리오 | 수정 |
//! |---|---|---|---|---|
//! | B1 | 같은 이름 자기닫힘 자식에서 depth 오계산 -> 컴파일 에러 | — | (이 파일 밖) | `open_tag_is_self_closing`로 자기닫힘은 depth를 늘리지 않는다 (`tests/syntax.rs`) |
//! | B2 | 세로 컨테이너의 **Row 형제**가 높이 대신 **폭**으로 계산돼 `height: fill` 예산을 먹는다 | -(폭-높이) = -272 / -260 | S02 · S04 · S26 | `intrinsic_main`이 부모 주축을 보고 교차축은 자식 **최댓값**으로 잰다 |
//! | B5 | `margin`이 **컨테이너가 아닌 자식**(Button 등)에 적용되지 않는다 | -6 (margin 6 무시) | S12 | 리프 위젯을 `outer_margin` 프레임으로 감싼다 |
//! | B6 | `height: fill` 컨테이너에 `max-height`가 걸리지 않는다 | +512 (40 상한 무시) | S17 | `apply_size`가 예산을 `min/max`로 clamp한다 |
//! | B7 | `wrap` 줄바꿈 사이에 `gap`이 적용되지 않는다 | -12 | S22 | `wrap`이면 `item_spacing.y`도 `gap` |
//! | B8 | `display: none` 노드의 해석 스타일이 `pass.styles`에 없다 | — | S14 (`scenario_css_is_parsed`) | 조기 반환 **전에** 스타일 기록 |
//!
//! 추가로 조사해 고친 flex 결함 (S28~S32로 잠금):
//! - 가변 자식이 `max-*`/`min-*`에 닿으면 남은 공간을 다른 가변 자식에게 **재분배** (S28 · S31)
//! - 가변이 상한에 묶여 남은 공간은 `justify`가 쓴다 (S32)
//! - `Button`/`Tab`/`Th`의 `width/height: fill`·`max-*`·`min-*` 반영 (S29 · S30)
//! - 컨테이너 **교차축 `fill`**이 주축 예산 때문에 통째로 무시되던 문제 (`apply_size`)
//! - `Input`이 세로 컨테이너에서 늘어나던 문제 (폭만 가변, 높이는 글자 높이)
//! - `Strong`/`Button`/`Tab` 내재 크기의 **이중 패딩**
//! - `Divider`/`Spinner`/`Progress`가 예산에서 0으로 취급되던 문제
//!
//! **B1은 이 파일에서 재현할 수 없다** (컴파일 타임). 회귀 테스트는
//! `tests/syntax.rs::nested_self_closing_same_tag_compiles_and_renders`가 잠근다.
//! 그래서 이 파일은 **항상 명시적 닫힘 태그**(`</Col>`)만 쓴다.

use elm_magic::style::{Palette, ResolvedStyle};
use std::cell::RefCell;

const WINDOW: egui::Vec2 = egui::vec2(800.0, 600.0);
/// 상댓값 판정 허용오차(px) — egui 반올림/서브픽셀 여유.
const TOL: f32 = 1.0;

/// 한 계측 지점 — 그 지점이 본 사각형.
#[derive(Clone, Debug)]
struct Probe {
    tag: String,
    rect: egui::Rect,
}

thread_local! {
    static PROBES: RefCell<Vec<Probe>> = const { RefCell::new(Vec::new()) };
}

/// `<Raw>`가 부르는 계측 훅.
fn record(tag: &'static str, ui: &egui::Ui) {
    PROBES.with(|p| {
        p.borrow_mut().push(Probe {
            tag: tag.to_string(),
            rect: ui.max_rect(),
        })
    });
}

/// 한 시나리오의 렌더 결과.
struct Measured {
    probes: Vec<Probe>,
    buttons: Vec<(String, egui::Rect)>,
    styles: Vec<(&'static str, ResolvedStyle)>,
    root: egui::Rect,
}

impl Measured {
    /// 계측 지점의 사각형 (없으면 패닉 — 시나리오 정의 오류를 빨리 잡는다).
    fn rect(&self, tag: &str) -> egui::Rect {
        self.probes
            .iter()
            .find(|p| p.tag == tag)
            .unwrap_or_else(|| panic!("계측 지점 `{tag}`가 없다: {:?}", self.tags()))
            .rect
    }

    fn tags(&self) -> Vec<String> {
        self.probes.iter().map(|p| p.tag.clone()).collect()
    }

    /// 라벨로 버튼 응답 사각형.
    fn button(&self, label: &str) -> egui::Rect {
        self.buttons
            .iter()
            .find(|(t, _)| t.contains(label))
            .unwrap_or_else(|| panic!("버튼 `{label}`이 없다"))
            .1
    }

    /// 태그의 **해석된** 스타일 (첫 번째) — CSS 파싱 여부 판정용.
    fn style_of(&self, tag: &str) -> Option<&ResolvedStyle> {
        self.styles.iter().find(|(t, _)| *t == tag).map(|(_, s)| s)
    }
}

fn measure<C: elm_magic::Component>(
    elm: &mut elm_magic::Ctx,
    props: &C::Props,
    window: egui::Vec2,
) -> Measured {
    PROBES.with(|p| p.borrow_mut().clear());
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, window)),
        ..Default::default()
    };
    let palette: Palette = Palette::dark();
    let mut buttons = Vec::new();
    let mut root = egui::Rect::NOTHING;
    let mut styles = Vec::new();
    let mut out = ctx.run_ui(input, |ui| {
        let tree = elm_magic::frame::<C>(elm, props);
        let pass = elm_magic_egui::render_with_palette(ui, &tree, &mut elm.arena, &palette);
        buttons.extend(pass.buttons.into_iter().map(|(t, r)| (t, r.rect)));
        styles = pass.styles;
        root = ui.max_rect();
    });
    out.textures_delta.clear();
    Measured {
        probes: PROBES.with(|p| p.borrow().clone()),
        buttons,
        styles,
        root,
    }
}

/// 시나리오 판정 한 줄 — 예측/계측을 **문자열(상댓값)** 로 남긴다.
struct Check {
    predicted: String,
    measured: String,
    /// 예측 - 계측 (delta). 0에 가까울수록 예측이 맞다.
    delta: f32,
    pass: bool,
}

impl Check {
    /// delta 하나로 판정 — 허용오차 `TOL`.
    fn delta(predicted: String, measured: String, delta: f32) -> Self {
        let pass = delta.abs() <= TOL;
        Self {
            predicted,
            measured,
            delta,
            pass,
        }
    }

    /// 술어로 판정 — delta는 참고값(로그용).
    fn predicate(predicted: String, measured: String, delta: f32, pass: bool) -> Self {
        Self {
            predicted,
            measured,
            delta,
            pass,
        }
    }
}

/// 시나리오 하나 — 이름 + 실행기.
type Scenario = (&'static str, fn() -> Check);

fn line(name: &str, c: &Check) -> String {
    format!(
        "{:<40} {:>8.1}  {:<5} 예측 {} / 계측 {}",
        name,
        c.delta,
        if c.pass { "OK" } else { "BUG!" },
        c.predicted,
        c.measured
    )
}

// ── 시나리오용 CSS (freedf-gui 스타일과 무관 — 이 파일 안에서만) ──────────
elm_magic::css! {
    // 루트(세로) — gap만 선언 / 높이 fill 변형 / padding 변형
    .sc_root { gap: 8; }
    .sc_root_fill { height: fill; gap: 8; }
    .sc_root_pad { height: fill; gap: 8; padding: 10; }
    // 고정 크기 블록
    .sc_fix40 { height: 40; bg: surface; }
    .sc_fix20 { height: 20; bg: surface; }
    .sc_fixh200 { height: 200; bg: surface; }
    .sc_w300 { width: 300; bg: surface; }
    .sc_w300h60 { width: 300; height: 60; bg: surface; }
    // 형제 Row 3종 — 높이 없음 / 명시 height / min-height 만
    .sc_rowbar { gap: 4; bg: background; }
    .sc_rowbar_fix { height: 40; gap: 4; bg: background; }
    .sc_rowbar_minh { min-height: 40; gap: 4; bg: background; }
    // 채움 대상
    .sc_body_fill { height: fill; gap: 8; }
    .sc_body_fill_maxh40 { height: fill; max-height: 40; gap: 8; }
    .sc_fill { width: fill; height: fill; }
    .sc_fillw { width: fill; }
    .sc_fillh { height: fill; }
    .sc_fillmaxw100 { width: fill; max-width: 100; }
    .sc_btnfill { width: fill; min-height: 28; }
    .sc_fixh150 { height: 150; }
    .sc_fillminh100 { height: fill; min-height: 100; }
    // 컨테이닝 속성
    .sc_gap12 { gap: 12; }
    .sc_pad10 { padding: 10; }
    .sc_margin6 { margin: 6; }
    .sc_border2 { border-width: 2; border-color: border; padding: 4; }
    .sc_end { justify: end; }
    .sc_alignc { align: center; }
    .sc_aligne { align: end; }
    .sc_wrap { wrap: true; }
    .sc_none { display: none; height: 40; }
    .sc_hidden { visibility: hidden; height: 40; }
    .sc_minh40 { min-height: 40; bg: surface; }
    // 확정 크기 자식(계측 기준점) — 버튼으로 그려 응답 사각형을 읽는다
    .sc_btn300 { width: 300; min-height: 28; }
    .sc_btn200 { width: 200; min-height: 28; }
    .sc_btn100 { width: 100; min-height: 28; }
    .sc_btn60 { width: 60; min-height: 20; }
    .sc_fix100x40 { width: 100; height: 40; }
    .sc_fix100x28 { width: 100; min-height: 28; }
    .sc_margin6b { margin: 6; width: 100; height: 40; }
}

elm_magic::view! {
    /// S01 — Col + 고정 높이 Col 형제 + `height: fill` body (기준선).
    pub fn S01() {
        <Col class="sc_root">
            <Col class="sc_fix40"></Col>
            <Col class="sc_body_fill"><Raw>|ui: &mut egui::Ui| { crate::record("body", ui); }</Raw></Col>
        </Col>
    }

    /// S02 — 형제가 **Row**(높이 선언 없음).
    pub fn S02() {
        <Col class="sc_root">
            <Row class="sc_rowbar"><Button class="sc_btn300">"R"</Button></Row>
            <Col class="sc_body_fill"><Raw>|ui: &mut egui::Ui| { crate::record("body", ui); }</Raw></Col>
        </Col>
    }

    /// S03 — 형제 Row에 **명시 height: 40**.
    pub fn S03() {
        <Col class="sc_root">
            <Row class="sc_rowbar_fix"><Button class="sc_btn300">"R"</Button></Row>
            <Col class="sc_body_fill"><Raw>|ui: &mut egui::Ui| { crate::record("body", ui); }</Raw></Col>
        </Col>
    }

    /// S04 — 형제 Row에 **min-height: 40** 만.
    pub fn S04() {
        <Col class="sc_root">
            <Row class="sc_rowbar_minh"><Button class="sc_btn300">"R"</Button></Row>
            <Col class="sc_body_fill"><Raw>|ui: &mut egui::Ui| { crate::record("body", ui); }</Raw></Col>
        </Col>
    }

    /// S05 — Row 안 `width: fill` + 뒤 형제(자리 예약).
    pub fn S05() {
        <Row class="sc_w300">
            <Raw>|ui: &mut egui::Ui| { crate::record("row", ui); }</Raw>
            <Col class="sc_fill"></Col>
            <Button class="sc_btn100">"B"</Button>
        </Row>
    }

    /// S06 — 콘텐츠 크기 자식 Row의 `justify: end`.
    pub fn S06() {
        <Row class="sc_w300">
            <Raw>|ui: &mut egui::Ui| { crate::record("row", ui); }</Raw>
            <Row class="sc_end"><Button class="sc_btn100">"J"</Button></Row>
        </Row>
    }

    /// S07 — `wrap: true` + 넘치는 폭(3번째가 다음 줄).
    pub fn S07() {
        <Row class="sc_w300 sc_wrap">
            <Button class="sc_btn200">"W1"</Button>
            <Button class="sc_btn200">"W2"</Button>
            <Button class="sc_btn200">"W3"</Button>
        </Row>
    }

    /// S08 — Col `gap: 12` (세로 간격).
    pub fn S08() {
        <Col class="sc_gap12">
            <Button class="sc_fix100x28">"G1"</Button>
            <Button class="sc_fix100x28">"G2"</Button>
        </Col>
    }

    /// S09 — Row `gap: 12` (가로 간격).
    pub fn S09() {
        <Row class="sc_gap12">
            <Button class="sc_fix100x28">"H1"</Button>
            <Button class="sc_fix100x28">"H2"</Button>
        </Row>
    }

    /// S10 — Row `padding: 10` (내부 폭이 줄어드는가).
    pub fn S10() {
        <Row class="sc_w300 sc_pad10">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Button class="sc_btn100">"P"</Button>
        </Row>
    }

    /// S11 — Row `align: center` (교차축 가운데).
    pub fn S11() {
        <Row class="sc_w300h60 sc_alignc">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Button class="sc_btn60">"C"</Button>
        </Row>
    }

    /// S12 — 자식 `margin: 6` (형제 간격에 더해지는가).
    pub fn S12() {
        <Col class="sc_root">
            <Button class="sc_fix100x40">"F"</Button>
            <Button class="sc_margin6b">"M"</Button>
        </Col>
    }

    /// S13 — 컨테이너 `border-width: 2` + `padding: 4` (박스 모델: 테두리도 자리를 먹는가).
    pub fn S13() {
        <Row class="sc_w300 sc_border2">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Button class="sc_btn100">"BD"</Button>
        </Row>
    }
}

elm_magic::view! {
    /// S14 — `display: none` 자식은 자리도 안 먹는다.
    pub fn S14() {
        <Col class="sc_root">
            <Button class="sc_fix100x40">"D1"</Button>
            <Col class="sc_none"></Col>
            <Button class="sc_fix100x40">"D2"</Button>
        </Col>
    }

    /// S15 — `visibility: hidden` 자식은 자리를 지킨다.
    pub fn S15() {
        <Col class="sc_root">
            <Button class="sc_fix100x40">"V1"</Button>
            <Col class="sc_hidden"></Col>
            <Button class="sc_fix100x40">"V2"</Button>
        </Col>
    }

    /// S16 — `min-height` 만 있는 컨테이너의 높이.
    pub fn S16() {
        <Col class="sc_root">
            <Button class="sc_fix100x28">"BEFORE"</Button>
            <Col class="sc_minh40"></Col>
            <Button class="sc_fix100x28">"AFTER"</Button>
        </Col>
    }

    /// S17 — `height: fill` + `max-height` (상한이 걸리는가).
    pub fn S17() {
        <Col class="sc_root_fill">
            <Col class="sc_fix40"></Col>
            <Col class="sc_body_fill_maxh40"><Raw>|ui: &mut egui::Ui| { crate::record("body", ui); }</Raw></Col>
        </Col>
    }

    /// S18 — `height: fill` 형제 2개가 남은 높이를 나눠 갖는가.
    pub fn S18() {
        <Col class="sc_root_fill">
            <Col class="sc_fill"><Raw>|ui: &mut egui::Ui| { crate::record("a", ui); }</Raw></Col>
            <Col class="sc_fill"><Raw>|ui: &mut egui::Ui| { crate::record("b", ui); }</Raw></Col>
            <Raw>|ui: &mut egui::Ui| { crate::record("root", ui); }</Raw>
        </Col>
    }

    /// S19 — Col 안 자식의 `width: fill` (교차축 채움).
    pub fn S19() {
        <Col class="sc_w300">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Col class="sc_fillw"><Raw>|ui: &mut egui::Ui| { crate::record("child", ui); }</Raw></Col>
        </Col>
    }

    /// S20 — Row 안 자식의 `height: fill` (교차축 채움).
    pub fn S20() {
        <Row class="sc_w300h60">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Col class="sc_fillh"><Raw>|ui: &mut egui::Ui| { crate::record("child", ui); }</Raw></Col>
        </Row>
    }

    /// S21 — Col `justify: end` (마지막 자식이 바닥에 붙는가).
    pub fn S21() {
        <Col class="sc_fixh200 sc_end">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Col class="sc_fix40"><Raw>|ui: &mut egui::Ui| { crate::record("child", ui); }</Raw></Col>
        </Col>
    }

    /// S22 — `wrap` + `gap: 12` (줄 간격에도 gap이 걸리는가).
    pub fn S22() {
        <Row class="sc_w300 sc_wrap sc_gap12">
            <Button class="sc_btn200">"L1"</Button>
            <Button class="sc_btn200">"L2"</Button>
            <Button class="sc_btn200">"L3"</Button>
        </Row>
    }

    /// S23 — 중첩 채움: Row `fill` 안의 fill Col 두 개가 폭을 나누는가.
    pub fn S23() {
        <Col class="sc_w300">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Row class="sc_fill">
                <Col class="sc_fill"><Raw>|ui: &mut egui::Ui| { crate::record("a", ui); }</Raw></Col>
                <Col class="sc_fill"><Raw>|ui: &mut egui::Ui| { crate::record("b", ui); }</Raw></Col>
            </Row>
        </Col>
    }

    /// S24 — 루트 `padding: 10` + `height: fill` body.
    pub fn S24() {
        <Col class="sc_root_pad">
            <Col class="sc_fix40"></Col>
            <Col class="sc_body_fill"><Raw>|ui: &mut egui::Ui| { crate::record("body", ui); }</Raw></Col>
        </Col>
    }

    /// S25 — `available_size()`를 먹는 `<Raw>` 뒤 자식의 폭.
    pub fn S25() {
        <Row class="sc_w300">
            <Raw>|ui: &mut egui::Ui| { crate::record("row", ui); }</Raw>
            <Raw>|ui: &mut egui::Ui| { let size = ui.available_size(); ui.allocate_space(size); }</Raw>
            <Col class="sc_fillw"><Raw>|ui: &mut egui::Ui| { crate::record("after", ui); }</Raw></Col>
        </Row>
    }

    /// S26 — Row 형제가 body **뒤**에 오는 순서(버그의 순서 의존성 확인).
    pub fn S26() {
        <Col class="sc_root_fill">
            <Col class="sc_body_fill"><Raw>|ui: &mut egui::Ui| { crate::record("body", ui); }</Raw></Col>
            <Row class="sc_rowbar"><Button class="sc_btn300">"R"</Button></Row>
        </Col>
    }

    /// S27 — Row `align: end` (교차축 끝).
    pub fn S27() {
        <Row class="sc_w300h60 sc_aligne">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Button class="sc_btn60">"E"</Button>
        </Row>
    }

    /// S28 — `height: fill; max-height: 40` 형제가 상한에 닿으면 남은 공간을 재분배.
    pub fn S28() {
        <Col class="sc_root_fill">
            <Col class="sc_body_fill_maxh40"><Raw>|ui: &mut egui::Ui| { crate::record("a", ui); }</Raw></Col>
            <Col class="sc_body_fill"><Raw>|ui: &mut egui::Ui| { crate::record("b", ui); }</Raw></Col>
        </Col>
    }

    /// S29 — Row 안 Button `width: fill`이 뒤 형제를 남기고 채운다.
    pub fn S29() {
        <Row class="sc_w300">
            <Button class="sc_btnfill">"F"</Button>
            <Button class="sc_btn100">"B"</Button>
        </Row>
    }

    /// S30 — Row 안 Col `width: fill; max-width: 100`에 상한이 걸린다.
    pub fn S30() {
        <Row class="sc_w300">
            <Col class="sc_fillmaxw100"><Raw>|ui: &mut egui::Ui| { crate::record("a", ui); }</Raw></Col>
            <Button class="sc_btn100">"B"</Button>
        </Row>
    }

    /// S31 — `height: fill; min-height: 100` 형제가 하한을 지키고 남은 공간을 재분배.
    pub fn S31() {
        <Col class="sc_fixh150">
            <Col class="sc_fillminh100"><Raw>|ui: &mut egui::Ui| { crate::record("a", ui); }</Raw></Col>
            <Col class="sc_fill"><Raw>|ui: &mut egui::Ui| { crate::record("b", ui); }</Raw></Col>
        </Col>
    }

    /// S32 — `justify: end` + 상한에 묶인 fill 자식 (남은 공간이 정렬에 쓰인다).
    pub fn S32() {
        <Col class="sc_fixh200 sc_end">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Col class="sc_body_fill_maxh40"><Raw>|ui: &mut egui::Ui| { crate::record("child", ui); }</Raw></Col>
        </Col>
    }
}

// ── 시나리오 실행기 — 예측을 **상댓값**으로 적고 delta 하나로 판정한다 ──────
//
// 표기: `창` = 창 높이(600), `형제합` = 앞선 형제들의 높이 합, `gap` = 컨테이너 gap.

fn s01() -> Check {
    let m = measure::<S01>(&mut elm_magic::Ctx::default(), &S01Props::default(), WINDOW);
    let body = m.rect("body");
    let want = WINDOW.y - 40.0 - 8.0;
    Check::delta(
        format!("body.h == 창-40-gap = {want:.0}"),
        format!("{:.0}", body.height()),
        body.height() - want,
    )
}

fn s02() -> Check {
    let m = measure::<S02>(&mut elm_magic::Ctx::default(), &S02Props::default(), WINDOW);
    let body = m.rect("body");
    let row = m.button("R");
    let want = WINDOW.y - row.height() - 8.0;
    Check::delta(
        format!("body.h == 창-형제높이({:.0})-gap = {want:.0}", row.height()),
        format!("{:.0}", body.height()),
        body.height() - want,
    )
}

fn s03() -> Check {
    let m = measure::<S03>(&mut elm_magic::Ctx::default(), &S03Props::default(), WINDOW);
    let body = m.rect("body");
    let want = WINDOW.y - 40.0 - 8.0;
    Check::delta(
        format!("body.h == 창-40-gap = {want:.0}"),
        format!("{:.0}", body.height()),
        body.height() - want,
    )
}

fn s04() -> Check {
    let m = measure::<S04>(&mut elm_magic::Ctx::default(), &S04Props::default(), WINDOW);
    let body = m.rect("body");
    let want = WINDOW.y - 40.0 - 8.0;
    Check::delta(
        format!("body.h == 창-minh40-gap = {want:.0}"),
        format!("{:.0}", body.height()),
        body.height() - want,
    )
}

fn s05() -> Check {
    let m = measure::<S05>(&mut elm_magic::Ctx::default(), &S05Props::default(), WINDOW);
    let row = m.rect("row");
    let btn = m.button("B");
    let over = (btn.max.x - row.max.x).max(0.0);
    Check::delta(
        format!("fill 뒤 형제가 행 안에 남음 (right - row.right <= 0)"),
        format!(
            "right-row.right = {:+.1} (행 폭 {:.0})",
            btn.max.x - row.max.x,
            row.width()
        ),
        over,
    )
}

fn s06() -> Check {
    let m = measure::<S06>(&mut elm_magic::Ctx::default(), &S06Props::default(), WINDOW);
    let row = m.rect("row");
    let btn = m.button("J");
    Check::delta(
        format!("justify:end -> btn.right == row.right"),
        format!("delta = {:+.1}", btn.max.x - row.max.x),
        btn.max.x - row.max.x,
    )
}

fn s07() -> Check {
    let m = measure::<S07>(&mut elm_magic::Ctx::default(), &S07Props::default(), WINDOW);
    let w1 = m.button("W1");
    let w3 = m.button("W3");
    // 200px 자식은 300px 행에 하나씩만 들어간다 -> 3번째는 3번째 줄 = dy 2줄
    let want = 2.0 * w1.height();
    Check::delta(
        format!("wrap: 자식 3개가 각각 한 줄 (dy == 2*줄높이 {want:.0})"),
        format!("dy = {:.1}", w3.min.y - w1.min.y),
        (w3.min.y - w1.min.y) - want,
    )
}

fn s08() -> Check {
    let m = measure::<S08>(&mut elm_magic::Ctx::default(), &S08Props::default(), WINDOW);
    let g1 = m.button("G1");
    let g2 = m.button("G2");
    Check::delta(
        format!("Col gap:12 -> 세로 간격 == 12"),
        format!("간격 = {:.1}", g2.min.y - g1.max.y),
        (g2.min.y - g1.max.y) - 12.0,
    )
}

fn s09() -> Check {
    let m = measure::<S09>(&mut elm_magic::Ctx::default(), &S09Props::default(), WINDOW);
    let h1 = m.button("H1");
    let h2 = m.button("H2");
    Check::delta(
        format!("Row gap:12 -> 가로 간격 == 12"),
        format!("간격 = {:.1}", h2.min.x - h1.max.x),
        (h2.min.x - h1.max.x) - 12.0,
    )
}

fn s10() -> Check {
    let m = measure::<S10>(&mut elm_magic::Ctx::default(), &S10Props::default(), WINDOW);
    let inner = m.rect("inner");
    // content-box: `width: 300`은 **내용 상자** 폭이다 -> padding은 바깥으로 늘린다
    let want = 300.0;
    Check::delta(
        format!("content-box: padding은 내부 폭을 줄이지 않음 -> {want:.0}"),
        format!("내부 폭 = {:.0}", inner.width()),
        inner.width() - want,
    )
}

fn s11() -> Check {
    let m = measure::<S11>(&mut elm_magic::Ctx::default(), &S11Props::default(), WINDOW);
    let inner = m.rect("inner");
    let btn = m.button("C");
    Check::delta(
        format!("align:center -> 자식 중심 == 컨테이너 중심"),
        format!("dy = {:+.1}", btn.center().y - inner.center().y),
        btn.center().y - inner.center().y,
    )
}

fn s12() -> Check {
    let m = measure::<S12>(&mut elm_magic::Ctx::default(), &S12Props::default(), WINDOW);
    let f = m.button("F");
    let mb = m.button("M");
    let want = 8.0 + 6.0;
    Check::delta(
        format!("margin:6 -> 형제 간격 == gap+margin = {want:.0}"),
        format!("간격 = {:.1}", mb.min.y - f.max.y),
        (mb.min.y - f.max.y) - want,
    )
}

fn s13() -> Check {
    let m = measure::<S13>(&mut elm_magic::Ctx::default(), &S13Props::default(), WINDOW);
    let inner = m.rect("inner");
    // content-box: 테두리·패딩은 내용 상자 **바깥**이다 (CSS 기본값과 같음)
    let want = 300.0;
    Check::delta(
        format!("content-box: 테두리+패딩이 내부 폭을 줄이지 않음 -> {want:.0}"),
        format!("내부 폭 = {:.0}", inner.width()),
        inner.width() - want,
    )
}

fn s14() -> Check {
    let m = measure::<S14>(&mut elm_magic::Ctx::default(), &S14Props::default(), WINDOW);
    let d1 = m.button("D1");
    let d2 = m.button("D2");
    Check::delta(
        format!("display:none -> 자리 없음 (간격 == gap 8)"),
        format!("간격 = {:.1}", d2.min.y - d1.max.y),
        (d2.min.y - d1.max.y) - 8.0,
    )
}

fn s15() -> Check {
    let m = measure::<S15>(&mut elm_magic::Ctx::default(), &S15Props::default(), WINDOW);
    let v1 = m.button("V1");
    let v2 = m.button("V2");
    let want = 40.0 + 8.0 + 8.0;
    Check::delta(
        format!("visibility:hidden -> 자리 유지 (간격 == 40+gap+gap = {want:.0})"),
        format!("간격 = {:.1}", v2.min.y - v1.max.y),
        (v2.min.y - v1.max.y) - want,
    )
}

fn s16() -> Check {
    let m = measure::<S16>(&mut elm_magic::Ctx::default(), &S16Props::default(), WINDOW);
    let before = m.button("BEFORE");
    let after = m.button("AFTER");
    // gap(8) + minh(40) + gap(8)
    let want = 8.0 + 40.0 + 8.0;
    Check::delta(
        format!("min-height:40 -> 앞뒤 버튼 간격 == gap+40+gap = {want:.0}"),
        format!("간격 = {:.1}", after.min.y - before.max.y),
        (after.min.y - before.max.y) - want,
    )
}

fn s17() -> Check {
    let m = measure::<S17>(&mut elm_magic::Ctx::default(), &S17Props::default(), WINDOW);
    let body = m.rect("body");
    Check::delta(
        format!("fill+max-height:40 -> 40으로 상한"),
        format!("높이 = {:.0}", body.height()),
        body.height() - 40.0,
    )
}

fn s18() -> Check {
    let m = measure::<S18>(&mut elm_magic::Ctx::default(), &S18Props::default(), WINDOW);
    let a = m.rect("a");
    let b = m.rect("b");
    // 마지막 프로브가 본 max_rect는 남은 공간 -> 루트 내부 높이는 a+b+gap으로 역산
    let equal = (a.height() - b.height()).abs() <= TOL;
    let fits = ((a.height() + b.height() + 8.0) - (WINDOW.y - 0.0)).abs() <= 8.0;
    Check::predicate(
        format!("fill 형제 2개: 높이 동일 + 합+gap == 루트"),
        format!(
            "a={:.0} b={:.0} 합+gap={:.0} 창={:.0}",
            a.height(),
            b.height(),
            a.height() + b.height() + 8.0,
            WINDOW.y
        ),
        a.height() - b.height(),
        equal && fits,
    )
}

fn s19() -> Check {
    let m = measure::<S19>(&mut elm_magic::Ctx::default(), &S19Props::default(), WINDOW);
    let inner = m.rect("inner");
    let child = m.rect("child");
    Check::delta(
        format!("Col 안 width:fill -> 자식 폭 == 부모 내부 폭"),
        format!("자식 {:.0} vs 부모 {:.0}", child.width(), inner.width()),
        child.width() - inner.width(),
    )
}

fn s20() -> Check {
    let m = measure::<S20>(&mut elm_magic::Ctx::default(), &S20Props::default(), WINDOW);
    let inner = m.rect("inner");
    let child = m.rect("child");
    Check::delta(
        format!("Row 안 height:fill -> 자식 높이 == 부모 내부 높이"),
        format!("자식 {:.0} vs 부모 {:.0}", child.height(), inner.height()),
        child.height() - inner.height(),
    )
}

fn s21() -> Check {
    let m = measure::<S21>(&mut elm_magic::Ctx::default(), &S21Props::default(), WINDOW);
    let inner = m.rect("inner");
    let child = m.rect("child");
    Check::delta(
        format!("Col justify:end -> 자식 바닥 == 컨테이너 내부 바닥"),
        format!("dy = {:+.1}", inner.max.y - child.max.y),
        inner.max.y - child.max.y,
    )
}

fn s22() -> Check {
    let m = measure::<S22>(&mut elm_magic::Ctx::default(), &S22Props::default(), WINDOW);
    let l1 = m.button("L1");
    let l2 = m.button("L2");
    let want = l1.height() + 12.0;
    Check::delta(
        format!("wrap 줄 간격 == 줄높이+gap = {want:.0}"),
        format!("dy = {:.1}", l2.min.y - l1.min.y),
        (l2.min.y - l1.min.y) - want,
    )
}

fn s23() -> Check {
    let m = measure::<S23>(&mut elm_magic::Ctx::default(), &S23Props::default(), WINDOW);
    let inner = m.rect("inner");
    let a = m.rect("a");
    let b = m.rect("b");
    let equal = (a.width() - b.width()).abs() <= TOL;
    let fits = ((a.width() + b.width()) - inner.width()).abs() <= TOL;
    Check::predicate(
        format!("중첩 fill: 두 자식 폭 동일 + 합 == 부모 내부"),
        format!(
            "a={:.0} b={:.0} 합={:.0} 부모={:.0}",
            a.width(),
            b.width(),
            a.width() + b.width(),
            inner.width()
        ),
        a.width() - b.width(),
        equal && fits,
    )
}

fn s24() -> Check {
    let m = measure::<S24>(&mut elm_magic::Ctx::default(), &S24Props::default(), WINDOW);
    let body = m.rect("body");
    let want = WINDOW.y - 20.0 - 40.0 - 8.0;
    Check::delta(
        format!("루트 padding:10 -> body.h == 창-20-40-gap = {want:.0}"),
        format!("높이 = {:.0}", body.height()),
        body.height() - want,
    )
}

fn s25() -> Check {
    let m = measure::<S25>(&mut elm_magic::Ctx::default(), &S25Props::default(), WINDOW);
    let after = m.rect("after");
    // 0.7.5에서는 quirk가 재현되지 않는다: 뒤 자식이 행의 남은 폭을 그대로 받는다.
    Check::delta(
        format!("Raw 뒤 자식이 남은 폭을 받는다 (quirk 해소) -> 300"),
        format!("폭 = {:.0}", after.width()),
        after.width() - 300.0,
    )
}

fn s26() -> Check {
    let m = measure::<S26>(&mut elm_magic::Ctx::default(), &S26Props::default(), WINDOW);
    let body = m.rect("body");
    let row = m.button("R");
    let want = WINDOW.y - row.height() - 8.0;
    Check::delta(
        format!(
            "형제 Row가 뒤에 와도 body.h == 창-형제높이({:.0})-gap = {want:.0}",
            row.height()
        ),
        format!("높이 = {:.0}", body.height()),
        body.height() - want,
    )
}

fn s27() -> Check {
    let m = measure::<S27>(&mut elm_magic::Ctx::default(), &S27Props::default(), WINDOW);
    let inner = m.rect("inner");
    let btn = m.button("E");
    Check::delta(
        format!("align:end -> 자식 바닥 == 컨테이너 내부 바닥"),
        format!("dy = {:+.1}", inner.max.y - btn.max.y),
        inner.max.y - btn.max.y,
    )
}

fn s28() -> Check {
    let m = measure::<S28>(&mut elm_magic::Ctx::default(), &S28Props::default(), WINDOW);
    let a = m.rect("a");
    let b = m.rect("b");
    // a는 max-height:40에 묶이고, 남은 (600-40-8)=552는 b가 먹는다.
    let want_a = 40.0;
    let want_b = WINDOW.y - want_a - 8.0;
    let pass = (a.height() - want_a).abs() <= TOL && (b.height() - want_b).abs() <= TOL;
    Check::predicate(
        format!("fill+max-height 형제: 상한 뒤 남은 공간 재분배 (a={want_a:.0} b={want_b:.0})"),
        format!("a={:.0} b={:.0}", a.height(), b.height()),
        a.height() - want_a,
        pass,
    )
}

fn s29() -> Check {
    let m = measure::<S29>(&mut elm_magic::Ctx::default(), &S29Props::default(), WINDOW);
    let f = m.button("F");
    let b = m.button("B");
    // F는 남은 폭(300-100)을 채우고, B가 그 뒤에 붙는다.
    let want = 300.0 - 100.0;
    let pass = (f.width() - want).abs() <= TOL
        && (b.min.x - f.max.x).abs() <= TOL
        && (b.max.x - 300.0).abs() <= TOL;
    Check::predicate(
        format!("Button width:fill -> 남은 폭 {want:.0} 채움 + 뒤 형제 유지"),
        format!(
            "f={:.0} b.x={:.0} b.right={:.0}",
            f.width(),
            b.min.x,
            b.max.x
        ),
        f.width() - want,
        pass,
    )
}

fn s30() -> Check {
    let m = measure::<S30>(&mut elm_magic::Ctx::default(), &S30Props::default(), WINDOW);
    let a = m.rect("a");
    let b = m.button("B");
    // fill Col에 max-width:100 상한이 걸린다.
    let pass = (a.width() - 100.0).abs() <= TOL && (b.min.x - a.max.x).abs() <= TOL;
    Check::predicate(
        format!("fill Col max-width:100 -> 상한 100"),
        format!("a={:.0} b.x={:.0}", a.width(), b.min.x),
        a.width() - 100.0,
        pass,
    )
}

fn s31() -> Check {
    let m = measure::<S31>(&mut elm_magic::Ctx::default(), &S31Props::default(), WINDOW);
    let a = m.rect("a");
    let b = m.rect("b");
    // 컨테이너 150 — a는 min-height 100, b가 나머지 50을 먹는다.
    let pass = (a.height() - 100.0).abs() <= TOL && (b.height() - 50.0).abs() <= TOL;
    Check::predicate(
        format!("fill+min-height 형제: 하한 100 지키고 나머지 50 재분배"),
        format!("a={:.0} b={:.0}", a.height(), b.height()),
        a.height() - 100.0,
        pass,
    )
}

fn s32() -> Check {
    let m = measure::<S32>(&mut elm_magic::Ctx::default(), &S32Props::default(), WINDOW);
    let inner = m.rect("inner");
    let child = m.rect("child");
    // 자식은 max-height 40으로 묶이고, justify:end가 남은 160을 앞에 넣는다.
    let pass = (child.height() - 40.0).abs() <= TOL && (inner.max.y - child.max.y).abs() <= TOL;
    Check::predicate(
        format!("justify:end + fill max-height:40 -> 자식 40, 바닥 정렬"),
        format!(
            "h={:.0} 바닥차 = {:+.1}",
            child.height(),
            inner.max.y - child.max.y
        ),
        child.height() - 40.0,
        pass,
    )
}

// ── 매트릭스 ────────────────────────────────────────────────────────────

fn scenarios() -> Vec<Scenario> {
    vec![
        ("S01 Col + 고정Col형제 + fill body", s01),
        ("S02 Col + Row형제(높이없음) + fill", s02),
        ("S03 Col + Row형제(height:40) + fill", s03),
        ("S04 Col + Row형제(min-height:40) + fill", s04),
        ("S05 Row: width:fill + 뒤 형제 예약", s05),
        ("S06 Row: 콘텐츠 자식 justify:end", s06),
        ("S07 Row: wrap:true 넘침", s07),
        ("S08 Col: gap:12 세로", s08),
        ("S09 Row: gap:12 가로", s09),
        ("S10 Row: padding:10", s10),
        ("S11 Row: align:center", s11),
        ("S12 Col: 자식 margin:6", s12),
        ("S13 Row: border-width:2 + padding:4", s13),
        ("S14 Col: display:none 자식", s14),
        ("S15 Col: visibility:hidden 자식", s15),
        ("S16 Col: min-height:40", s16),
        ("S17 Col: fill + max-height:40", s17),
        ("S18 Col: fill 형제 2개 분배", s18),
        ("S19 Col 안 width:fill (교차축)", s19),
        ("S20 Row 안 height:fill (교차축)", s20),
        ("S21 Col: justify:end", s21),
        ("S22 Row: wrap + gap 줄간격", s22),
        ("S23 중첩 fill (Row 안 두 fill Col)", s23),
        ("S24 루트 padding:10 + fill body", s24),
        ("S25 available_size 소비 뒤 자식", s25),
        ("S26 Row형제가 body 뒤(순서 의존)", s26),
        ("S27 Row: align:end", s27),
        ("S28 Col: fill+max-height 형제 재분배", s28),
        ("S29 Row: Button width:fill", s29),
        ("S30 Row: Col width:fill + max-width", s30),
        ("S31 Col: fill+min-height 형제 재분배", s31),
        ("S32 Col: justify:end + fill max-height", s32),
    ]
}

/// 매트릭스 전체를 표로 찍고, 예측과 다른 행(=버그)이 있으면 실패한다.
#[test]
fn container_matrix_prediction_vs_measurement() {
    let rows: Vec<(String, Check)> = scenarios()
        .iter()
        .map(|(n, f)| (line(n, &f()), f()))
        .collect();
    let _ = &rows;
    eprintln!("\n== elm-magic 0.7.6 컨테이너/컨테이닝 CSS 매트릭스 (예측 vs 계측) ==");
    for (n, f) in scenarios() {
        eprintln!("{}", line(n, &f()));
    }
    let bugs: Vec<String> = scenarios()
        .iter()
        .filter_map(|(n, f)| {
            let c = f();
            (!c.pass).then(|| line(n, &c))
        })
        .collect();
    eprintln!("== 버그 {} / 전체 {} ==\n", bugs.len(), scenarios().len());
    // 하네스 sanity: 루트 ui가 창 크기를 봤는가 (계측이 헛돌면 판정이 무의미하다)
    let sanity = measure::<S01>(&mut elm_magic::Ctx::default(), &S01Props::default(), WINDOW);
    assert!(
        (sanity.root.width() - WINDOW.x).abs() <= TOL
            && (sanity.root.height() - WINDOW.y).abs() <= TOL,
        "하네스가 창 크기를 못 봤다: {:?}",
        sanity.root
    );
    assert!(
        bugs.is_empty(),
        "예측과 계측이 다르다(=버그) {}건:\n{}",
        bugs.len(),
        bugs.join("\n")
    );
}

/// 계측이 어긋날 때 **CSS 파싱 문제**인지 먼저 배제한다 — 대표 시나리오의 해석값 확인.
#[test]
fn scenario_css_is_parsed() {
    let cases: Vec<(&str, fn() -> Check)> = vec![];
    let _ = cases;

    let m = measure::<S08>(&mut elm_magic::Ctx::default(), &S08Props::default(), WINDOW);
    let col = m.style_of("col").expect("S08 Col 스타일");
    assert_eq!(col.gap, Some(12.0), "gap:12가 해석되지 않았다");

    let m = measure::<S11>(&mut elm_magic::Ctx::default(), &S11Props::default(), WINDOW);
    let row = m.style_of("row").expect("S11 Row 스타일");
    assert_eq!(row.align, Some(elm_magic::style::Align::Center));

    let m = measure::<S21>(&mut elm_magic::Ctx::default(), &S21Props::default(), WINDOW);
    let col = m.style_of("col").expect("S21 Col 스타일");
    assert_eq!(col.justify, Some(elm_magic::style::Align::End));

    let m = measure::<S22>(&mut elm_magic::Ctx::default(), &S22Props::default(), WINDOW);
    let row = m.style_of("row").expect("S22 Row 스타일");
    assert_eq!(row.wrap, Some(true), "wrap:true가 해석되지 않았다");

    let m = measure::<S13>(&mut elm_magic::Ctx::default(), &S13Props::default(), WINDOW);
    let row = m.style_of("row").expect("S13 Row 스타일");
    assert_eq!(row.border_width, Some(2.0));
    assert_eq!(row.padding, Some(elm_magic::style::Edges::splat(4.0)));

    let m = measure::<S14>(&mut elm_magic::Ctx::default(), &S14Props::default(), WINDOW);
    let hidden = m
        .styles
        .iter()
        .filter(|(t, _)| *t == "col")
        .map(|(_, s)| s.clone())
        .find(|s| s.is_display_none())
        .expect("display:none Col 스타일");
    assert!(hidden.is_display_none());
}
