//! elm-magic 0.7.6 **flex 시나리오 매트릭스** — 예측 vs 계측.
//!
//! `elm_fill_bug_test.rs`와 **같은 하네스/판정 규약**을 쓴다. 그 파일이 컨테이너/
//! 컨테이닝 CSS(박스 모델·display/visibility·gap)를 다룬다면, 이 파일은
//! **flex 배분**에 집중한다: 분배·상/하한·교차축 채움·wrap·정렬·리프 위젯의 fill.
//!
//! ## 방법
//!
//! 1. 시나리오마다 **예측**을 상댓값(창 대비 비율, 형제 대비 delta)으로 적는다.
//! 2. 헤드리스로 렌더하고 `<Raw>`/`Button` 응답에서 **계측**한다.
//! 3. `|예측 - 계측| > 허용오차`면 **버그**다 — 매트릭스가 그 행을 `BUG!`로 보고한다.
//!
//! 절대 픽셀값은 **판정에 쓰지 않는다**: "창 - 형제 합 - gap" 같은 **delta**나
//! "부모 내부 대비"로만 판정한다. 그래야 창 크기를 바꿔도 같은 결론이 나온다.
//!
//! ## 계측 지점 규약 (elm_fill_bug_test.rs와 동일)
//!
//! - 컨테이너의 **내부 사각형**: 그 컨테이너의 **첫 자식** `<Raw>`의 `ui.max_rect()`.
//! - 자식 하나의 사각형: 그 자식 **안에** `<Raw>`.
//! - **커서 위치**: `<Raw>`의 `ui.cursor().min` — "다음 위젯이 놓일 자리"라서
//!   그 앞 위젯(`Spinner`/`Divider`/`Progress`/`Input`)이 소비한 크기를 역산한다.
//! - px로 확정된 크기: `<Button>`으로 그려 `pass.buttons` 사각형을 읽는다.
//!
//! ## 판정 보조 — CSS가 파싱됐는지 먼저 본다
//!
//! 계측이 어긋날 때 원인은 (a) CSS 미해석, (b) 레이아웃 오배분 둘이다.
//! `pass.styles`의 `style_of(tag)`로 (a)를 먼저 배제한다 — 남으면 (b) = 버그다.
//!
//! ## 이 파일이 잠그는 flex 동작
//!
//! | 시나리오 | 예측 |
//! |---|---|
//! | F01 Row `width: fill` 2개 균등 분배 | 300 → 150 + 150 |
//! | F02 Col `height: fill` 2개 균등 분배 | 600 − gap → 296 + 296 |
//! | F03 가변 형제 `max-height` 상한 후 재분배 | 40 + 552 |
//! | F04 가변 형제 `min-height` 하한 지키고 재분배 | 100 + 50 |
//! | F05 `justify: end` + 상한에 묶인 fill | 자식 40, 바닥 정렬 |
//! | F06 `Button width: fill` | 남은 200 채움 + 뒤 형제 유지 |
//! | F07 `Button min-width` | 버튼 폭 120 |
//! | F08 `wrap` + gap 줄 간격 | 줄 높이 + 12 |
//! | F09 `wrap` + gap 0 줄 간격 | 줄 높이 |
//! | F10 Row 자식 `margin` 이중 합 | 형제 간격 12 |
//! | F11 content-box padding/border | 내부 폭 300 유지 |
//! | F12 Row `align: center` | 자식 중심 == 행 중심 |
//! | F13 Row 안 `height: fill` | == 행 내부 높이 |
//! | F14 Col 안 `width: fill` | == 열 내부 폭 |
//! | F15 Row `align: end` | 자식 바닥 == 행 내부 바닥 |
//! | F16 Col 안 `Input` | 세로로 늘어나지 않음 |
//! | F17 `Divider border-width` | 자리 2 |
//! | F18 `Spinner width` | 자리 20 |
//! | F19 `Progress height` | 자리 12 |
//! | F20 Col `justify: center` | 자식 중심 == 열 중심 |

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

/// `<Raw>`가 부르는 계측 훅 — `ui.max_rect()` = 아직 아무것도 안 그린 내부 영역.
fn record(tag: &'static str, ui: &egui::Ui) {
    PROBES.with(|p| {
        p.borrow_mut().push(Probe {
            tag: tag.to_string(),
            rect: ui.max_rect(),
        })
    });
}

/// `<Raw>`가 부르는 커서 훅 — "다음 위젯이 놓일 자리".
///
/// 위젯 자체는 응답 사각형을 노출하지 않으므로(Spinner/Divider/Progress/Input),
/// 그 **앞**에 커서를 재고 **뒤**에 커서를 재어 소비 크기를 역산한다.
fn record_cursor(tag: &'static str, ui: &egui::Ui) {
    PROBES.with(|p| {
        p.borrow_mut().push(Probe {
            tag: tag.to_string(),
            rect: egui::Rect::from_min_size(ui.cursor().min, egui::Vec2::ZERO),
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
// ── flex 시나리오용 CSS (이 파일 안에서만) ───────────────────────────────
elm_magic::css! {
    // 루트/고정 크기
    .fx_h150 { height: 150; }
    .fx_h200 { height: 200; }
    .fx_root_fill { height: fill; gap: 8; }
    .fx_w300 { width: 300; }
    .fx_w300h60 { width: 300; height: 60; }
    // 가변
    .fx_fillw { width: fill; }
    .fx_fillh { height: fill; }
    .fx_fillmaxh40 { height: fill; max-height: 40; }
    .fx_fillminh100 { height: fill; min-height: 100; }
    .fx_btnfill { width: fill; min-height: 28; }
    .fx_btnmin { min-width: 120; min-height: 28; }
    // px 확정(계측 기준점) — 버튼
    .fx_btn200 { width: 200; min-height: 28; }
    .fx_btn100 { width: 100; min-height: 28; }
    .fx_btn40 { width: 40; min-height: 40; }
    .fx_btn20 { width: 40; min-height: 20; }
    .fx_margin6 { margin: 6; width: 100; min-height: 28; }
    // 컨테이닝 속성
    .fx_pad10 { padding: 10; }
    .fx_border2 { border-width: 2; border-color: border; }
    .fx_gap12 { gap: 12; }
    .fx_wrap { wrap: true; }
    .fx_end { justify: end; }
    .fx_center { justify: center; }
    .fx_alignc { align: center; }
    .fx_aligne { align: end; }
    // 위젯 크기
    .fx_div { border-width: 2; }
    .fx_spin { width: 20; }
    .fx_prog { height: 12; }
}

elm_magic::view! {
    /// F01 — Row `width: fill` 두 개가 폭을 균등 분배.
    pub fn F01() {
        <Row class="fx_w300">
            <Col class="fx_fillw"><Raw>|ui: &mut egui::Ui| { crate::record("a", ui); }</Raw></Col>
            <Col class="fx_fillw"><Raw>|ui: &mut egui::Ui| { crate::record("b", ui); }</Raw></Col>
        </Row>
    }

    /// F02 — Col `height: fill` 두 개가 높이를 균등 분배.
    pub fn F02() {
        <Col class="fx_root_fill">
            <Col class="fx_fillh"><Raw>|ui: &mut egui::Ui| { crate::record("a", ui); }</Raw></Col>
            <Col class="fx_fillh"><Raw>|ui: &mut egui::Ui| { crate::record("b", ui); }</Raw></Col>
        </Col>
    }

    /// F03 — 가변 형제가 `max-height`에 닿으면 남은 공간을 재분배.
    pub fn F03() {
        <Col class="fx_root_fill">
            <Col class="fx_fillmaxh40"><Raw>|ui: &mut egui::Ui| { crate::record("a", ui); }</Raw></Col>
            <Col class="fx_fillh"><Raw>|ui: &mut egui::Ui| { crate::record("b", ui); }</Raw></Col>
        </Col>
    }

    /// F04 — 가변 형제가 `min-height`를 지키고 남은 공간을 재분배.
    pub fn F04() {
        <Col class="fx_h150">
            <Col class="fx_fillminh100"><Raw>|ui: &mut egui::Ui| { crate::record("a", ui); }</Raw></Col>
            <Col class="fx_fillh"><Raw>|ui: &mut egui::Ui| { crate::record("b", ui); }</Raw></Col>
        </Col>
    }

    /// F05 — `justify: end` + 상한에 묶인 fill.
    pub fn F05() {
        <Col class="fx_h200 fx_end">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Col class="fx_fillmaxh40"><Raw>|ui: &mut egui::Ui| { crate::record("child", ui); }</Raw></Col>
        </Col>
    }
/// F06 — `Button`의 `width: fill`이 뒤 형제를 남기고 채운다.
    pub fn F06() {
        <Row class="fx_w300">
            <Button class="fx_btnfill">"F"</Button>
            <Button class="fx_btn100">"B"</Button>
        </Row>
    }

    /// F07 — `Button`의 `min-width`가 버튼 크기를 만든다.
    pub fn F07() {
        <Row class="fx_w300">
            <Button class="fx_btnmin">"M"</Button>
            <Button class="fx_btn100">"B"</Button>
        </Row>
    }

    /// F08 — `wrap` 줄바꿈 줄 간격에 gap이 적용된다.
    pub fn F08() {
        <Row class="fx_w300 fx_wrap fx_gap12">
            <Button class="fx_btn200">"W1"</Button>
            <Button class="fx_btn200">"W2"</Button>
            <Button class="fx_btn200">"W3"</Button>
        </Row>
    }

    /// F09 — `wrap` + gap 0 → 줄 간격은 줄 높이뿐.
    pub fn F09() {
        <Row class="fx_w300 fx_wrap">
            <Button class="fx_btn200">"N1"</Button>
            <Button class="fx_btn200">"N2"</Button>
            <Button class="fx_btn200">"N3"</Button>
        </Row>
    }

    /// F10 — Row 자식 `margin`이 형제 간격에 더해진다 (양쪽 6 + 6).
    pub fn F10() {
        <Row class="fx_w300">
            <Button class="fx_margin6">"A"</Button>
            <Button class="fx_margin6">"B"</Button>
        </Row>
    }

    /// F11 — content-box: padding/border가 내부 폭을 줄이지 않는다.
    pub fn F11() {
        <Row class="fx_w300 fx_pad10 fx_border2">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Button class="fx_btn100">"P"</Button>
        </Row>
    }

    /// F12 — Row `align: center` (교차축 중심).
    pub fn F12() {
        <Row class="fx_w300h60 fx_alignc">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Button class="fx_btn20">"C"</Button>
        </Row>
    }

    /// F13 — Row 안 자식의 `height: fill` (교차축 채움).
    pub fn F13() {
        <Row class="fx_w300h60">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Col class="fx_fillh"><Raw>|ui: &mut egui::Ui| { crate::record("child", ui); }</Raw></Col>
        </Row>
    }

    /// F14 — Col 안 자식의 `width: fill` (교차축 채움).
    pub fn F14() {
        <Col class="fx_w300">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Col class="fx_fillw"><Raw>|ui: &mut egui::Ui| { crate::record("child", ui); }</Raw></Col>
        </Col>
    }
/// F15 — Row `align: end` (교차축 끝).
    pub fn F15() {
        <Row class="fx_w300h60 fx_aligne">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Button class="fx_btn20">"E"</Button>
        </Row>
    }

    /// F16 — Col 안 `Input`은 세로로 늘어나지 않는다 (폭만 가변).
    pub fn F16() {
        <Col class="fx_h150">
            <Input />
            <Raw>|ui: &mut egui::Ui| { crate::record_cursor("after_input", ui); }</Raw>
            <Col class="fx_fillh"><Raw>|ui: &mut egui::Ui| { crate::record("body", ui); }</Raw></Col>
        </Col>
    }

    /// F17 — `Divider`의 `border-width`(두께)가 자리를 먹는다.
    pub fn F17() {
        <Col class="fx_h150">
            <Button class="fx_btn20">"D1"</Button>
            <Divider class="fx_div" />
            <Button class="fx_btn20">"D2"</Button>
        </Col>
    }

    /// F18 — `Spinner`의 `width`가 자리를 먹는다.
    pub fn F18() {
        <Col class="fx_h150">
            <Button class="fx_btn20">"S1"</Button>
            <Spinner class="fx_spin" />
            <Button class="fx_btn20">"S2"</Button>
        </Col>
    }

    /// F19 — `Progress`의 `height`가 자리를 먹는다.
    pub fn F19() {
        <Col class="fx_h150">
            <Button class="fx_btn20">"G1"</Button>
            <Progress class="fx_prog" value={0.5} />
            <Button class="fx_btn20">"G2"</Button>
        </Col>
    }

    /// F20 — Col `justify: center` (주축 중심).
    pub fn F20() {
        <Col class="fx_h200 fx_center">
            <Raw>|ui: &mut egui::Ui| { crate::record("inner", ui); }</Raw>
            <Button class="fx_btn20">"C"</Button>
        </Col>
    }
}

// ── 시나리오 실행기 — 예측을 **상댓값**으로 적고 delta/술어로 판정한다 ──────

fn f01() -> Check {
    let m = measure::<F01>(&mut elm_magic::Ctx::default(), &F01Props::default(), WINDOW);
    let a = m.rect("a");
    let b = m.rect("b");
    let pass = (a.width() - 150.0).abs() <= TOL
        && (b.width() - 150.0).abs() <= TOL
        && (a.width() + b.width() - 300.0).abs() <= TOL;
    Check::predicate(
        String::from("Row fill×2: 300 → 150 + 150"),
        format!("a={:.0} b={:.0}", a.width(), b.width()),
        a.width() - b.width(),
        pass,
    )
}

fn f02() -> Check {
    let m = measure::<F02>(&mut elm_magic::Ctx::default(), &F02Props::default(), WINDOW);
    let a = m.rect("a");
    let b = m.rect("b");
    let want = (WINDOW.y - 8.0) / 2.0;
    let pass = (a.height() - want).abs() <= TOL && (b.height() - want).abs() <= TOL;
    Check::predicate(
        format!("Col fill×2: 600-gap → {want:.0} + {want:.0}"),
        format!("a={:.0} b={:.0}", a.height(), b.height()),
        a.height() - b.height(),
        pass,
    )
}

fn f03() -> Check {
    let m = measure::<F03>(&mut elm_magic::Ctx::default(), &F03Props::default(), WINDOW);
    let a = m.rect("a");
    let b = m.rect("b");
    let pass = (a.height() - 40.0).abs() <= TOL && (b.height() - 552.0).abs() <= TOL;
    Check::predicate(
        String::from("fill max-height:40 상한 후 재분배 → 40 + 552"),
        format!("a={:.0} b={:.0}", a.height(), b.height()),
        a.height() - 40.0,
        pass,
    )
}

fn f04() -> Check {
    let m = measure::<F04>(&mut elm_magic::Ctx::default(), &F04Props::default(), WINDOW);
    let a = m.rect("a");
    let b = m.rect("b");
    let pass = (a.height() - 100.0).abs() <= TOL && (b.height() - 50.0).abs() <= TOL;
    Check::predicate(
        String::from("fill min-height:100 하한 후 재분배 → 100 + 50"),
        format!("a={:.0} b={:.0}", a.height(), b.height()),
        a.height() - 100.0,
        pass,
    )
}

fn f05() -> Check {
    let m = measure::<F05>(&mut elm_magic::Ctx::default(), &F05Props::default(), WINDOW);
    let inner = m.rect("inner");
    let child = m.rect("child");
    let pass = (child.height() - 40.0).abs() <= TOL && (inner.max.y - child.max.y).abs() <= TOL;
    Check::predicate(
        String::from("justify:end + fill max-height:40 → 40, 바닥 정렬"),
        format!(
            "h={:.0} 바닥차={:+.1}",
            child.height(),
            inner.max.y - child.max.y
        ),
        child.height() - 40.0,
        pass,
    )
}

fn f06() -> Check {
    let m = measure::<F06>(&mut elm_magic::Ctx::default(), &F06Props::default(), WINDOW);
    let f = m.button("F");
    let b = m.button("B");
    let pass = (f.width() - 200.0).abs() <= TOL
        && (b.min.x - f.max.x).abs() <= TOL
        && (b.max.x - 300.0).abs() <= TOL;
    Check::predicate(
        String::from("Button width:fill → 남은 200 + 뒤 형제 유지"),
        format!("f={:.0} b.x={:.0} right={:.0}", f.width(), b.min.x, b.max.x),
        f.width() - 200.0,
        pass,
    )
}

fn f07() -> Check {
    let m = measure::<F07>(&mut elm_magic::Ctx::default(), &F07Props::default(), WINDOW);
    let mm = m.button("M");
    let b = m.button("B");
    let pass = (mm.width() - 120.0).abs() <= TOL && (b.min.x - 120.0).abs() <= TOL;
    Check::predicate(
        String::from("Button min-width:120 → 폭 120"),
        format!("m={:.0} b.x={:.0}", mm.width(), b.min.x),
        mm.width() - 120.0,
        pass,
    )
}

fn f08() -> Check {
    let m = measure::<F08>(&mut elm_magic::Ctx::default(), &F08Props::default(), WINDOW);
    let w1 = m.button("W1");
    let w2 = m.button("W2");
    let want = w1.height() + 12.0;
    Check::delta(
        format!("wrap 줄 간격 == 줄높이+gap = {want:.0}"),
        format!("dy = {:.1}", w2.min.y - w1.min.y),
        (w2.min.y - w1.min.y) - want,
    )
}

fn f09() -> Check {
    let m = measure::<F09>(&mut elm_magic::Ctx::default(), &F09Props::default(), WINDOW);
    let n1 = m.button("N1");
    let n2 = m.button("N2");
    Check::delta(
        format!("wrap gap 0 → 줄 간격 == 줄높이 = {:.0}", n1.height()),
        format!("dy = {:.1}", n2.min.y - n1.min.y),
        (n2.min.y - n1.min.y) - n1.height(),
    )
}

fn f10() -> Check {
    let m = measure::<F10>(&mut elm_magic::Ctx::default(), &F10Props::default(), WINDOW);
    let a = m.button("A");
    let b = m.button("B");
    let pass = (b.min.x - a.max.x - 12.0).abs() <= TOL && (a.min.x - 6.0).abs() <= TOL;
    Check::predicate(
        String::from("Row margin:6 양쪽 → 형제 간격 12"),
        format!("gap={:.1} a.x={:.1}", b.min.x - a.max.x, a.min.x),
        b.min.x - a.max.x - 12.0,
        pass,
    )
}
fn f11() -> Check {
    let m = measure::<F11>(&mut elm_magic::Ctx::default(), &F11Props::default(), WINDOW);
    let inner = m.rect("inner");
    Check::delta(
        String::from("content-box: padding+border가 내부 폭을 줄이지 않음 → 300"),
        format!("내부 폭 = {:.0}", inner.width()),
        inner.width() - 300.0,
    )
}

fn f12() -> Check {
    let m = measure::<F12>(&mut elm_magic::Ctx::default(), &F12Props::default(), WINDOW);
    let inner = m.rect("inner");
    let btn = m.button("C");
    Check::delta(
        String::from("align:center → 자식 중심 == 행 중심"),
        format!("dy = {:+.1}", btn.center().y - inner.center().y),
        btn.center().y - inner.center().y,
    )
}

fn f13() -> Check {
    let m = measure::<F13>(&mut elm_magic::Ctx::default(), &F13Props::default(), WINDOW);
    let inner = m.rect("inner");
    let child = m.rect("child");
    Check::delta(
        String::from("Row 안 height:fill → 자식 높이 == 행 내부 높이"),
        format!(
            "자식 {:.0} vs 행 내부 {:.0}",
            child.height(),
            inner.height()
        ),
        child.height() - inner.height(),
    )
}

fn f14() -> Check {
    let m = measure::<F14>(&mut elm_magic::Ctx::default(), &F14Props::default(), WINDOW);
    let inner = m.rect("inner");
    let child = m.rect("child");
    Check::delta(
        String::from("Col 안 width:fill → 자식 폭 == 열 내부 폭"),
        format!("자식 {:.0} vs 열 내부 {:.0}", child.width(), inner.width()),
        child.width() - inner.width(),
    )
}

fn f15() -> Check {
    let m = measure::<F15>(&mut elm_magic::Ctx::default(), &F15Props::default(), WINDOW);
    let inner = m.rect("inner");
    let btn = m.button("E");
    Check::delta(
        String::from("align:end → 자식 바닥 == 행 내부 바닥"),
        format!("dy = {:+.1}", inner.max.y - btn.max.y),
        inner.max.y - btn.max.y,
    )
}

fn f16() -> Check {
    let m = measure::<F16>(&mut elm_magic::Ctx::default(), &F16Props::default(), WINDOW);
    let input_h = m.rect("after_input").min.y;
    let body = m.rect("body");
    // Input이 세로로 늘어나면 input_h가 150/2=75 근처가 된다 — 작아야 한다.
    let pass = input_h < 60.0 && body.height() > 100.0;
    Check::predicate(
        String::from("Col 안 Input은 늘지 않음 (input_h < 60, body가 나머지를 먹음)"),
        format!("input_h={:.0} body_h={:.0}", input_h, body.height()),
        input_h - 20.0,
        pass,
    )
}

fn f17() -> Check {
    let m = measure::<F17>(&mut elm_magic::Ctx::default(), &F17Props::default(), WINDOW);
    let d1 = m.button("D1");
    let d2 = m.button("D2");
    Check::delta(
        String::from("Divider border-width:2 → 자리 2"),
        format!("간격 = {:.1}", d2.min.y - d1.max.y),
        (d2.min.y - d1.max.y) - 2.0,
    )
}

fn f18() -> Check {
    let m = measure::<F18>(&mut elm_magic::Ctx::default(), &F18Props::default(), WINDOW);
    let s1 = m.button("S1");
    let s2 = m.button("S2");
    Check::delta(
        String::from("Spinner width:20 → 자리 20"),
        format!("간격 = {:.1}", s2.min.y - s1.max.y),
        (s2.min.y - s1.max.y) - 20.0,
    )
}

fn f19() -> Check {
    let m = measure::<F19>(&mut elm_magic::Ctx::default(), &F19Props::default(), WINDOW);
    let g1 = m.button("G1");
    let g2 = m.button("G2");
    Check::delta(
        String::from("Progress height:12 → 자리 12"),
        format!("간격 = {:.1}", g2.min.y - g1.max.y),
        (g2.min.y - g1.max.y) - 12.0,
    )
}

fn f20() -> Check {
    let m = measure::<F20>(&mut elm_magic::Ctx::default(), &F20Props::default(), WINDOW);
    let inner = m.rect("inner");
    let btn = m.button("C");
    Check::delta(
        String::from("justify:center → 자식 중심 == 열 중심"),
        format!("dy = {:+.1}", inner.center().y - btn.center().y),
        inner.center().y - btn.center().y,
    )
}
// ── 매트릭스 ────────────────────────────────────────────────────────────

fn scenarios() -> Vec<Scenario> {
    vec![
        ("F01 Row: width:fill 2개 균등", f01),
        ("F02 Col: height:fill 2개 균등", f02),
        ("F03 Col: fill max-height 상한 재분배", f03),
        ("F04 Col: fill min-height 하한 재분배", f04),
        ("F05 Col: justify:end + fill max-height", f05),
        ("F06 Row: Button width:fill", f06),
        ("F07 Row: Button min-width", f07),
        ("F08 Row: wrap + gap 줄간격", f08),
        ("F09 Row: wrap gap 0", f09),
        ("F10 Row: Button margin 양쪽", f10),
        ("F11 Row: padding+border content-box", f11),
        ("F12 Row: align:center", f12),
        ("F13 Row: height:fill 교차축", f13),
        ("F14 Col: width:fill 교차축", f14),
        ("F15 Row: align:end", f15),
        ("F16 Col: Input 세로 비확장", f16),
        ("F17 Col: Divider 두께 자리", f17),
        ("F18 Col: Spinner 크기 자리", f18),
        ("F19 Col: Progress 높이 자리", f19),
        ("F20 Col: justify:center", f20),
    ]
}

/// 매트릭스 전체를 표로 찍고, 예측과 다른 행(=버그)이 있으면 실패한다.
#[test]
fn flex_matrix_prediction_vs_measurement() {
    eprintln!("\n== elm-magic flex 매트릭스 (예측 vs 계측) ==");
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
    eprintln!(
        "== flex 버그 {} / 전체 {} ==\n",
        bugs.len(),
        scenarios().len()
    );
    // 하네스 sanity: 루트 ui가 창 크기를 봤는가
    let sanity = measure::<F01>(&mut elm_magic::Ctx::default(), &F01Props::default(), WINDOW);
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

/// 계측이 어긋날 때 **CSS 파싱 문제**인지 먼저 배제한다 — flex 선언의 해석 확인.
#[test]
fn flex_css_is_parsed() {
    let m = measure::<F08>(&mut elm_magic::Ctx::default(), &F08Props::default(), WINDOW);
    let row = m.style_of("row").expect("F08 Row 스타일");
    assert_eq!(row.gap, Some(12.0), "gap:12가 해석되지 않았다");
    assert_eq!(row.wrap, Some(true), "wrap:true가 해석되지 않았다");

    let m = measure::<F03>(&mut elm_magic::Ctx::default(), &F03Props::default(), WINDOW);
    let capped = m
        .styles
        .iter()
        .filter(|(t, _)| *t == "col")
        .map(|(_, s)| *s)
        .find(|s| s.max_height == Some(40.0))
        .expect("max-height:40 Col 스타일");
    assert_eq!(capped.max_height, Some(40.0));

    let m = measure::<F05>(&mut elm_magic::Ctx::default(), &F05Props::default(), WINDOW);
    let col = m.style_of("col").expect("F05 Col 스타일");
    assert_eq!(col.justify, Some(elm_magic::style::Align::End));

    let m = measure::<F13>(&mut elm_magic::Ctx::default(), &F13Props::default(), WINDOW);
    let fillh = m
        .styles
        .iter()
        .filter(|(t, _)| *t == "col")
        .map(|(_, s)| *s)
        .find(|s| s.height == Some(elm_magic::style::Len::Fill))
        .expect("height:fill Col 스타일");
    assert_eq!(fillh.height, Some(elm_magic::style::Len::Fill));
}
