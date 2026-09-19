//! egui 어터 (사양서 7.2).
//!
//! 어터는 라이브러리 내부에 있고, 클라이언트는 egui의 이름조차 모른다.
//! `render`는 `Element` 트리를 egui 위젯으로 그리고, 클릭/입력을 아레나로
//! 전달한다. `<Raw>`는 어댑터 핸들(`&mut egui::Ui`)을 받는 유일한 탈출구다.
//!
//! ## 스타일 (사양서 6.1~6.3)
//!
//! 해석은 코어가 끝낸다 — 여기서는 `ResolvedStyle`을 egui 값으로 **옮기기만**
//! 한다. 그래서 스타일 계약의 대부분은 egui 없이 `tests/style.rs`에서 검증된다.
//!
//! 어댑터가 코어에 주는 것:
//! - **조상 경로** → 후손(`.card Button`)·자식(`Col > Row`) 셀터 매칭
//! - **부모 스타일** → 상속(`color`/`font-size`/…)
//! - **상태** → `:hover` `:active` `:focus` `:disabled`
//!
//! `:hover`는 노드 사각형을 egui 메모리에 기록해 **다음 프레임에** 판정한다
//! (egui는 이전 프레임 rect로 상호작용을 계산하는 방식이라 1프레임 지연이
//! 시각적으로 느껴지지 않는다). `:disabled`는 리먼트 prop에서 온다.
//!
//! 스타일이 적용되는 태그: `Col` `Row` `Text` `Strong` `Button` `Banner`
//! `Tab` `Th` `Td` `Check` `Spinner` `Divider` `Progress` `Modal`.
//!
//! ## 0.8 확장 속성
//!
//! 코어가 해석하는 62개 속성 중 어댑터가 그리는 것: `flex-direction` `flex-grow`
//! `align-self` `padding-*`/`margin-*`(개별 변) `border-style`/`border-*-width`
//! `overflow` `aspect-ratio` `white-space` `text-overflow` `max-lines` `scale`
//! `z-index` `pointer-events`.
//!
//! 근사가 있는 것 (egui 표현 수단 한계):
//! - `border-style`의 `dashed`/`dotted`는 실선으로, 개별 `border-*-width`는
//!   네 변이 같은 stroke라 **최댓값**으로 그린다 (`none`은 테두리를 없앤다).
//! - `max-lines`는 egui에 줄 수 제한이 없어 **잘라내기**로 근사한다.
//! - `scale`은 노드 좌상단 기준 **시각 변환**으로, `z-index > 0`은 전경
//!   레이어로 근사한다. `rotate`는 egui `TSTransform`에 회전이 없어 반영하지 않는다.
//! - `pointer-events: none`은 클릭 핸들러와 커서를 막는다(호버 색은 남는다).
//! - `flex-shrink`는 코어가 값을 해석하지만 배분에는 아직 쓰지 않는다.
//!
//! ## 스타일 리셋 (기본값 없음)
//!
//! 어댑터는 그리기 전에 egui의 기본 스타일을 **전부 리셋**한다 —
//! [the-new-css-reset](https://github.com/elad2412/the-new-css-reset)의
//! `* { all: unset; display: revert; }`와 같은 출발점이다.
//! 즉 **보이는 것은 전부 `css!`가 선언한 것뿐**이고, egui가 기본으로 칠하던
//! 채움·테두리·여백·간격·라운딩·확장·그림자·글자 크기 계단은 모두 사라진다.
//! ([`reset_style_of`] / [`reset_style`])
//!
//! `display: revert`에 해당하는 것만 남긴다: 레이아웃(가로/세로/줄바꿈)과 상호작용.
//! `css!` 어휘로 **다시 칠할 수 없는 최소 기능**은 예외로 유지한다 — 글자색
//! (`fg_stroke`), 캐럿, 텍스트 선택, 하이퍼링크. 반대로 체크박스 박스나 스피너
//! 크기처럼 표현 수단이 없는 장식은 리셋되므로, 필요하면 `width`/`font-size` 같은
//! 선언으로 지정한다.

use elm_magic::style::{
    Align as StyleAlign, BorderStyle, Color, Cursor, Direction, Edges, Len, Overflow, Palette,
    ResolvedStyle, State, Token,
};
use elm_magic::Arena;
use elm_magic::{
    BannerEl, ButtonEl, CheckEl, ColEl, DividerEl, Element, FragmentEl, InputEl, ModalEl,
    ProgressEl, RawEl, RowEl, SpinnerEl, StrongEl, TabEl, TdEl, TextAreaEl, TextEl, ThEl, Widget,
};

/// 한 패스가 만든 위젯 정보.
pub struct Pass {
    /// 그려진 버튼: (라벨, 응답)
    pub buttons: Vec<(String, egui::Response)>,
    /// 그려진 체크박스: (라벨, 응답)
    pub checks: Vec<(String, egui::Response)>,
    /// **스타일이 적용된** 엘리먼트: (태그, 확정 스타일).
    ///
    /// egui 내부를 들여다보지 않고도 스타일이 전달됐는지 검증할 수 있다.
    pub styles: Vec<(&'static str, ResolvedStyle)>,
}

impl Pass {
    /// 태그의 스타일 (첫 번째) — 어댑터 테스트용.
    pub fn style_of(&self, tag: &str) -> Option<&ResolvedStyle> {
        self.styles.iter().find(|(t, _)| *t == tag).map(|(_, s)| s)
    }
}

/// 트리를 egui로 렌더링하고 상호작용을 아레나로 전달한다.
///
/// 팔레트는 egui의 밝기 설정(`Visuals::dark_mode`)을 따른다.
pub fn render(ui: &mut egui::Ui, tree: &Element, arena: &mut Arena) -> Pass {
    let palette = if ui.visuals().dark_mode {
        Palette::dark()
    } else {
        Palette::light()
    };
    render_with_palette(ui, tree, arena, &palette)
}

/// 팔레트를 지정해 렌더한다 (사양서 6.3 — 테마는 팔레트다).
///
/// 그리기 전에 egui 기본 스타일을 리셋한다([`reset_style_of`]). 리셋은 **이 렌더의
/// 스코프 안에서만** 적용되므로, 호출자의 다른 egui 위젯 스타일은 건드리지 않는다.
pub fn render_with_palette(
    ui: &mut egui::Ui,
    tree: &Element,
    arena: &mut Arena,
    palette: &Palette,
) -> Pass {
    let mut walk = Walk {
        pass: Pass {
            buttons: Vec::new(),
            checks: Vec::new(),
            styles: Vec::new(),
        },
        ancestors: Vec::new(),
        inherited: Vec::new(),
        node: 0,
        palette,
        main_budget: None,
    };
    ui.scope(|ui| {
        reset_style(ui, palette);
        render_el(&mut walk, ui, tree, arena);
    });
    walk.pass
}

/// 순회 상태 — 조상 경로 · 상속 · 노드 번호.
struct Walk<'a> {
    pass: Pass,
    /// 가까운 조상부터 (`ancestors[0]` = 부모).
    ancestors: Vec<&'a Element>,
    /// 상속 스택 (부모의 확정 스타일).
    inherited: Vec<ResolvedStyle>,
    /// 이번 프레임 노드 순번 — 메모리 키를 안정적으로 만든다.
    node: u32,
    palette: &'a Palette,
    /// 부모가 배분한 **주축 크기**(px) — `width/height: fill`과 `justify` 컨테이너가 쓴다.
    /// 뒤 형제의 몫을 예약한 결과라서 형제를 밀어내지 않는다.
    main_budget: Option<f32>,
}

impl<'a> Walk<'a> {
    /// 이 노드의 메모리 키.
    fn key(&self) -> egui::Id {
        egui::Id::new(("elm-magic-style", self.node))
    }

    /// 부모 스타일 (상속용).
    fn parent_style(&self) -> Option<&ResolvedStyle> {
        self.inherited.last()
    }
}

/// 노드의 지난 프레임 정보 (hover/focus 판정용).
#[derive(Clone, Copy, Debug)]
struct NodeMemory {
    rect: egui::Rect,
    focused: bool,
}

/// 지난 프레임 기록으로 상호작용 상태를 만든다.
fn read_state(ctx: &egui::Context, id: egui::Id, disabled: bool) -> State {
    let Some(mem) = ctx.memory(|m| m.data.get_temp::<NodeMemory>(id)) else {
        return State::new(false, false, false, disabled);
    };
    let hovered = ctx
        .pointer_hover_pos()
        .is_some_and(|p| mem.rect.contains(p));
    let active = hovered && ctx.input(|i| i.pointer.any_down());
    State::new(hovered, active, mem.focused, disabled)
}

/// 이번 프레임 결과를 다음 프레임을 위해 기록한다.
fn write_state(ctx: &egui::Context, id: egui::Id, rect: egui::Rect, focused: bool) {
    if rect.is_positive() && rect.is_finite() {
        ctx.memory_mut(|m| m.data.insert_temp(id, NodeMemory { rect, focused }));
    }
}

// ─ 스타일 → egui ────────────────────────────────────────────

fn color32(color: Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

fn margin(edges: Edges) -> egui::Margin {
    egui::Margin {
        left: edges.left as i8,
        right: edges.right as i8,
        top: edges.top as i8,
        bottom: edges.bottom as i8,
    }
}

fn radius(value: f32) -> egui::CornerRadius {
    egui::CornerRadius::same(value.clamp(0.0, 255.0) as u8)
}

fn align_of(a: Option<StyleAlign>) -> egui::Align {
    match a {
        Some(StyleAlign::Center) => egui::Align::Center,
        Some(StyleAlign::End) => egui::Align::Max,
        _ => egui::Align::Min,
    }
}

// ─ 0.8 확장 속성 → 실효 값 ─────────────────────────────────

/// 숏핸드(`padding`/`margin`)와 개별 `*-top/right/bottom/left`를 합친다.
///
/// 개별 값이 있으면 그 변만 덮어쓴다 (`padding: 8; padding-left: 4`).
fn combine_edges(
    base: Option<Edges>,
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    left: Option<f32>,
) -> Option<Edges> {
    let any = top.is_some() || right.is_some() || bottom.is_some() || left.is_some();
    match (base, any) {
        (None, false) => None,
        (None, true) => Some(Edges::new(
            top.unwrap_or(0.0),
            right.unwrap_or(0.0),
            bottom.unwrap_or(0.0),
            left.unwrap_or(0.0),
        )),
        (Some(b), _) => Some(Edges::new(
            top.unwrap_or(b.top),
            right.unwrap_or(b.right),
            bottom.unwrap_or(b.bottom),
            left.unwrap_or(b.left),
        )),
    }
}

/// 실효 `padding` — 숏핸드 + `padding-*`.
fn padding_of(style: &ResolvedStyle) -> Option<Edges> {
    combine_edges(
        style.padding,
        style.padding_top,
        style.padding_right,
        style.padding_bottom,
        style.padding_left,
    )
}

/// 실효 `margin` — 숏핸드 + `margin-*`.
fn margin_of(style: &ResolvedStyle) -> Option<Edges> {
    combine_edges(
        style.margin,
        style.margin_top,
        style.margin_right,
        style.margin_bottom,
        style.margin_left,
    )
}

/// 실효 테두리 두께 — `border-width` + `border-*-width`, `border-style: none`이면 없음.
///
/// egui `Frame`의 stroke는 네 변이 같은 두께라 개별 두께는 **최댓값**으로 근사한다.
fn border_of(style: &ResolvedStyle) -> Option<f32> {
    if style.border_style == Some(BorderStyle::None) {
        return None;
    }
    let declared = style.border_width.is_some()
        || style.border_top_width.is_some()
        || style.border_right_width.is_some()
        || style.border_bottom_width.is_some()
        || style.border_left_width.is_some();
    if !declared {
        return None;
    }
    let per_side = [
        style.border_top_width,
        style.border_right_width,
        style.border_bottom_width,
        style.border_left_width,
    ]
    .into_iter()
    .flatten()
    .fold(0.0f32, f32::max);
    Some(style.border_width.unwrap_or(0.0).max(0.0).max(per_side))
}

/// `overflow`가 자식을 자르는가 (`hidden`/`scroll`/`auto`).
fn clips_children(style: &ResolvedStyle) -> bool {
    matches!(
        style.overflow,
        Some(Overflow::Hidden) | Some(Overflow::Scroll) | Some(Overflow::Auto)
    )
}

/// `overflow`가 스크롤 영역을 만드는가 (`scroll`/`auto`).
fn scrolls_children(style: &ResolvedStyle) -> bool {
    matches!(
        style.overflow,
        Some(Overflow::Scroll) | Some(Overflow::Auto)
    )
}

/// `pointer-events`가 상호작용을 허용하는가 (미선언 = 허용).
fn interactive(style: &ResolvedStyle) -> bool {
    style.pointer_events != Some(false)
}

fn cursor_of(c: Cursor) -> egui::CursorIcon {
    match c {
        Cursor::Default => egui::CursorIcon::Default,
        Cursor::Pointer => egui::CursorIcon::PointingHand,
        Cursor::Text => egui::CursorIcon::Text,
        Cursor::Grab => egui::CursorIcon::Grab,
        Cursor::Grabbing => egui::CursorIcon::Grabbing,
        Cursor::Move => egui::CursorIcon::Move,
        Cursor::Crosshair => egui::CursorIcon::Crosshair,
        Cursor::NotAllowed => egui::CursorIcon::NotAllowed,
        Cursor::None => egui::CursorIcon::None,
    }
}

/// 리셋 후 기본 글자 크기 — `font-size`를 선언하지 않은 모든 텍스트.
pub const BASE_FONT_SIZE: f32 = 14.0;

/// egui의 기본 스타일을 **전부 리셋**한 `Style`을 만든다 (기본값 없음).
///
/// [the-new-css-reset](https://github.com/elad2412/the-new-css-reset)의
/// `* { all: unset; display: revert; }`에 대응한다. 장식은 전부 사라지고,
/// 보이는 것은 `css!` 선언뿐이다. `css!`로 다시 칠할 수 없는 최소 기능
/// (글자색 · 캐럿 · 텍스트 선택 · 하이퍼링크)만 남긴다.
pub fn reset_style_of(palette: &Palette) -> egui::Style {
    let text = color32(palette.get(Token::Text));
    let dim = color32(palette.get(Token::TextDim));
    let primary = palette.get(Token::Primary);
    let bg = palette.get(Token::Background);
    // 팔레트에서 밝기를 유도한다 (`background` 토큰) — `Palette`에는 dark 플래그가 없다.
    let dark = (u32::from(bg.r) * 299 + u32::from(bg.g) * 587 + u32::from(bg.b) * 114) / 1000 < 128;

    let mut style = egui::Style::default();

    // 애니메이션 없음 (`all: unset`)
    style.animation_time = 0.0;

    // 글자 크기 계단을 없앤다 (`h1~h6 { font-size: inherit }`).
    // 기본 맵의 **키를 그대로 두고 값만** 바꾼다 — egui가 TextStyle을 늘려도 안전하다.
    style.text_styles = style
        .text_styles
        .keys()
        .map(|ts| {
            let font = if *ts == egui::TextStyle::Monospace {
                egui::FontId::monospace(BASE_FONT_SIZE)
            } else {
                egui::FontId::proportional(BASE_FONT_SIZE)
            };
            (ts.clone(), font)
        })
        .collect();

    // 여백·간격 0 (`margin: 0; padding: 0; gap: 0`)
    let sp = &mut style.spacing;
    sp.item_spacing = egui::Vec2::ZERO;
    sp.window_margin = egui::Margin::ZERO;
    sp.button_padding = egui::Vec2::ZERO;
    sp.menu_margin = egui::Margin::ZERO;
    sp.menu_spacing = 0.0;
    sp.indent = 0.0;
    sp.interact_size = egui::Vec2::ZERO;
    sp.slider_width = 0.0;
    sp.slider_rail_height = 0.0;
    sp.combo_width = 0.0;
    sp.text_edit_width = 0.0;
    sp.icon_width = 0.0;
    sp.icon_width_inner = 0.0;
    sp.icon_spacing = 0.0;
    sp.extra_text_line_spacing = 0.0;
    sp.default_area_size = egui::Vec2::ZERO;
    sp.scroll.bar_width = 0.0;
    sp.scroll.bar_inner_margin = 0.0;
    sp.scroll.bar_outer_margin = 0.0;
    sp.scroll.handle_min_length = 0.0;
    sp.scroll.floating = false;

    // 표면: 아무것도 칠하지 않는다
    let v = &mut style.visuals;
    v.dark_mode = dark;
    v.panel_fill = egui::Color32::TRANSPARENT;
    v.window_fill = egui::Color32::TRANSPARENT;
    v.window_stroke = egui::Stroke::NONE;
    v.window_corner_radius = egui::CornerRadius::ZERO;
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.menu_corner_radius = egui::CornerRadius::ZERO;
    v.window_highlight_topmost = false;
    v.faint_bg_color = egui::Color32::TRANSPARENT;
    v.extreme_bg_color = egui::Color32::TRANSPARENT;
    v.text_edit_bg_color = Some(egui::Color32::TRANSPARENT);
    v.code_bg_color = egui::Color32::TRANSPARENT;
    v.striped = false;
    v.resize_corner_size = 0.0;
    v.weak_text_color = Some(dim);

    // 위젯 5상태 전부 리셋 — hover/active도 칠하지 않으므로 `:hover`를 선언해야 보인다
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_fill = egui::Color32::TRANSPARENT;
        w.weak_bg_fill = egui::Color32::TRANSPARENT;
        w.bg_stroke = egui::Stroke::NONE;
        w.corner_radius = egui::CornerRadius::ZERO;
        w.expansion = 0.0;
        w.fg_stroke = egui::Stroke::new(1.0, text);
    }

    // `css!`로 다시 칠할 수 없는 최소 기능 (기능이 사라지면 안 되는 것들)
    v.override_text_color = Some(text);
    v.hyperlink_color = color32(primary);
    v.warn_fg_color = color32(palette.get(Token::Warn));
    v.error_fg_color = color32(palette.get(Token::Error));
    v.text_cursor.stroke = egui::Stroke::new(1.0, text);
    v.selection.bg_fill =
        egui::Color32::from_rgba_unmultiplied(primary.r, primary.g, primary.b, 96);
    v.selection.stroke = egui::Stroke::new(1.0, text);

    style
}

/// `ui`의 스타일을 리셋된 것으로 **교체**한다 ([`reset_style_of`] 참고).
///
/// 어댑터는 [`render_with_palette`] 안에서 자동으로 부른다. 사용자가 직접 만든
/// egui 위젯도 같은 출발점에서 시작하려면 이 함수를 쓰면 된다.
pub fn reset_style(ui: &mut egui::Ui, palette: &Palette) {
    *ui.style_mut() = reset_style_of(palette);
}

/// 테이너 레이아웃 — `align`은 교차축(items)이다.
///
/// 주축 정렬(`justify`)은 여기서 하지 않는다: egui의 `Layout::main_align`은 자식 배치가
/// 아니라 **위젯 내부** 정렬(예: 버튼 글자)에 쓰인다. 남는 공간을 앞에 넣는 일은
/// [`container`]가 직접 한다.
fn layout_of(style: &ResolvedStyle, vertical: bool) -> egui::Layout {
    let cross = align_of(style.align);
    if vertical {
        egui::Layout::top_down(cross)
    } else {
        // `wrap: true` → egui의 main_wrap (0.7.4에는 읽지 않아 한 줄로 넘쳤다)
        egui::Layout::left_to_right(cross).with_main_wrap(style.wrap == Some(true))
    }
}

/// 배경·여백·모서리·테두리·그림자 → egui `Frame`.
fn frame_of(style: &ResolvedStyle, palette: &Palette) -> egui::Frame {
    let mut frame = egui::Frame::NONE;
    if let Some(bg) = style.bg {
        frame = frame.fill(color32(bg));
    }
    if let Some(padding) = padding_of(style) {
        frame = frame.inner_margin(margin(padding));
    }
    if let Some(outer) = margin_of(style) {
        frame = frame.outer_margin(margin(outer));
    }
    if let Some(r) = style.radius {
        frame = frame.corner_radius(radius(r));
    }
    if let Some(width) = border_of(style) {
        let color = style
            .border_color
            .unwrap_or_else(|| palette.get(Token::Border));
        frame = frame.stroke(egui::Stroke::new(width, color32(color)));
    }
    if let Some(s) = style.shadow {
        frame = frame.shadow(egui::Shadow {
            offset: [
                s.dx.clamp(-128.0, 127.0) as i8,
                s.dy.clamp(-128.0, 127.0) as i8,
            ],
            blur: s.blur.clamp(0.0, 255.0) as u8,
            spread: s.spread.clamp(0.0, 255.0) as u8,
            color: color32(style.shadow_color_or(palette)),
        });
    }
    frame
}

/// 글자 스타일 → `RichText` (`default_bold`는 태그 기본값).
fn rich(text: &str, style: &ResolvedStyle, default_bold: bool) -> egui::RichText {
    let text = match style.transform {
        Some(t) => t.apply(text),
        None => text.to_string(),
    };
    let mut out = egui::RichText::new(text);
    if let Some(color) = style.color {
        out = out.color(color32(color));
    }
    if let Some(size) = style.font_size {
        out = out.size(size);
    }
    if style.bold.unwrap_or(default_bold) {
        out = out.strong();
    }
    if style.italic.unwrap_or(false) {
        out = out.italics();
    }
    if style.mono.unwrap_or(false) {
        out = out.monospace();
    }
    if style.strike.unwrap_or(false) {
        out = out.strikethrough();
    }
    if style.underline.unwrap_or(false) {
        out = out.underline();
    }
    if let Some(lh) = style.line_height {
        out = out.line_height(Some(lh));
    }
    if let Some(ls) = style.letter_spacing {
        out = out.extra_letter_spacing(ls);
    }
    out
}

/// 글자 위젯 — `text-align`(`halign`)과 `truncate`까지 반영한다.
fn text_widget(
    ui: &mut egui::Ui,
    text: &str,
    style: &ResolvedStyle,
    default_bold: bool,
) -> egui::Response {
    let mut label = egui::Label::new(rich(text, style, default_bold));
    if let Some(a) = style.text_align {
        label = label.halign(align_of(Some(a)));
    }
    // `white-space: nowrap` — 줄바꿈하지 않는다.
    if style.nowrap == Some(true) {
        label = label.wrap_mode(egui::TextWrapMode::Extend);
    }
    // `text-overflow: ellipsis` / `truncate` / `max-lines`(egui는 줄 수 제한이
    // 없어 잘라내기로 근사한다).
    if style.truncate == Some(true)
        || style.ellipsis == Some(true)
        || style.max_lines.is_some_and(|n| n > 0)
    {
        label = label.truncate();
    }
    ui.add(label)
}

/// 컨테이너/위젯 크기 (`width`/`height`/`min`/`max`).
///
/// `budget`은 부모가 배분한 주축 크기다 — `fill`의 실제 값이 되고, 뒤 형제의 몫은
/// 이미 빠져 있다. 선언한 `width/height`(px)가 있으면 그쪽이 이긴다.
fn apply_size(ui: &mut egui::Ui, style: &ResolvedStyle, budget: Option<f32>) {
    // 이 시점의 `ui.layout()`은 **부모의** 주축이다 — 부모가 배분한 `budget`도 그
    // 축의 값이다. 그래서 예산은 주축에만 적용하고, 교차축의 `fill`은 여기서 따로
    // 계산한다(예전에는 예산이 있으면 교차축 `fill`이 통째로 무시됐다).
    let horizontal = ui.layout().main_dir().is_horizontal();
    let declared_main = if horizontal {
        style.width
    } else {
        style.height
    };
    let declared_cross = if horizontal {
        style.height
    } else {
        style.width
    };

    let main = match declared_main {
        // 선언한 px가 예산보다 우선한다.
        Some(Len::Px(v)) => Some(v),
        _ => match budget.filter(|b| b.is_finite()) {
            Some(b) => Some(b),
            // 예산이 없을 때만 남은 공간을 `fill`로 쓴다 (루트 요소 등).
            None => match declared_main {
                Some(Len::Fill) => {
                    let available = if horizontal {
                        ui.available_width()
                    } else {
                        ui.available_height()
                    };
                    available.is_finite().then_some(available)
                }
                _ => None,
            },
        },
    };
    let cross = match declared_cross {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Fill) => {
            let available = if horizontal {
                ui.available_height()
            } else {
                ui.available_width()
            };
            available.is_finite().then_some(available)
        }
        _ => None,
    };

    let (mut w, mut h) = if horizontal {
        (main, cross)
    } else {
        (cross, main)
    };
    // `aspect-ratio` — 한 축이 정해지면 다른 축을 비율로 채운다.
    if let Some(r) = style.aspect_ratio.filter(|r| *r > 0.0) {
        match (w, h) {
            (Some(wv), None) => h = Some(wv / r),
            (None, Some(hv)) => w = Some(hv * r),
            _ => {}
        }
    }
    // `max-*`는 예산까지 포함해 상한을 건다 — `set_height(예산)` 뒤에
    // `set_max_height(40)`을 부르면 egui가 `min <= max`라 상한을 무시했다(버그 B6).
    match w.map(|v| clamp_axis(v, style.min_width, style.max_width)) {
        Some(v) => ui.set_width(v),
        None => {
            if let Some(v) = style.min_width {
                ui.set_min_width(v);
            }
            if let Some(v) = style.max_width {
                ui.set_max_width(v);
            }
        }
    }
    match h.map(|v| clamp_axis(v, style.min_height, style.max_height)) {
        Some(v) => ui.set_height(v),
        None => {
            if let Some(v) = style.min_height {
                ui.set_min_height(v);
            }
            if let Some(v) = style.max_height {
                ui.set_max_height(v);
            }
        }
    }
}

/// 한 축 길이를 `min-*`/`max-*`로 확정한다 (`max`가 `min`보다 우선).
fn clamp_axis(desired: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let mut v = desired;
    if let Some(m) = min {
        v = v.max(m);
    }
    if let Some(m) = max {
        v = v.min(m);
    }
    v.max(0.0)
}

// ─ 주축 크기 측정 (fill 배분용) ──────────────────────────────

/// `Edges`의 주축 합 (가로면 좌우, 세로면 상하).
fn edges_main(edges: Option<Edges>, horizontal: bool) -> f32 {
    match edges {
        Some(e) if horizontal => e.left + e.right,
        Some(e) => e.top + e.bottom,
        None => 0.0,
    }
}

/// `padding`의 주축 합 (가로면 좌우, 세로면 상하).
fn padding_main(style: &ResolvedStyle, horizontal: bool) -> f32 {
    edges_main(padding_of(style), horizontal)
}

/// `margin`의 주축 합.
///
/// 컨테이너는 `Frame`의 `outer_margin`으로 이미 자리를 예약하지만, Button 같은
/// 리프 위젯은 `margin`이 무시됐다(버그 B5). 이제 리프도 바깥 여백 프레임을
/// 쓰므로 예산 계산에도 같은 마진을 더한다.
fn margin_main(style: &ResolvedStyle, horizontal: bool) -> f32 {
    edges_main(margin_of(style), horizontal)
}

/// 테두리가 주축 양쪽에 더하는 두께 (egui `Frame`은 stroke를 크기에 포함한다).
fn border_main(style: &ResolvedStyle, _horizontal: bool) -> f32 {
    border_of(style).unwrap_or(0.0).max(0.0) * 2.0
}

/// `intrinsic_main`과 같은 규칙으로 확정 스타일만 구한다.
fn resolve_style(walk: &Walk<'_>, el: &Element) -> ResolvedStyle {
    el.resolved_style_in(
        &walk.ancestors,
        State::new(false, false, false, el.is_disabled()),
        walk.parent_style(),
        walk.palette,
    )
}

/// 텍스트의 실제 크기(주축 방향) — egui 폰트로 잰다 (선언한 크기·스타일 반영).
fn text_extent(
    ui: &egui::Ui,
    text: &str,
    style: &ResolvedStyle,
    bold: bool,
    horizontal: bool,
) -> f32 {
    let rich = rich(text, style, bold);
    let size = egui::WidgetText::from(rich)
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
        .size();
    if horizontal {
        size.x
    } else {
        size.y
    }
}

/// 위젯 하나의 **주축 방향 고정 크기** 추정. `None` = 가변(fill/justify/입력의 폭).
///
/// `horizontal`은 **부모의 주축**이다. 컨테이너는 자기 주축이 부모와 같으면
/// 자식들을 주축 방향으로 **합**하고, 다르면(= 부모가 교차축을 잰다) 자식들의
/// **최댓값**을 쓴다 — 예전에는 항상 자기 주축으로만 재서 `Col` 안의 `Row` 형제가
/// 높이 대신 폭을 예산에 넣었다(버그 B2 · S02/S04/S26).
fn intrinsic_main(walk: &Walk<'_>, ui: &egui::Ui, el: &Element, horizontal: bool) -> Option<f32> {
    let style = resolve_style(walk, el);
    if style.is_display_none() {
        return Some(0.0);
    }

    let declared = if horizontal {
        style.width
    } else {
        style.height
    };
    let floor = axis_min(&style, horizontal);
    let ceil = axis_max(&style, horizontal);
    let pad = padding_main(&style, horizontal);
    let border = border_main(&style, horizontal);
    let margin = margin_main(&style, horizontal);

    // 내용 상자(content-box)를 마진까지 포함한 외곽 크기로 바꾼다.
    // `min-*`는 하한, `max-*`는 상한이다 (max가 min보다 우선).
    let finish = |content: f32| -> f32 {
        let mut v = content + pad + border;
        if let Some(f) = floor {
            v = v.max(f);
        }
        if let Some(c) = ceil {
            v = v.min(c);
        }
        v + margin
    };

    match declared {
        // 선언한 px가 있으면 그쪽이 이긴다.
        Some(Len::Px(v)) => return Some(finish(v)),
        // `fill`은 부모 예산을 받아야 하므로 가변.
        Some(Len::Fill) => return None,
        _ => {}
    }

    // `flex-grow`가 있으면 부모 주축의 남는 공간을 받는 가변 자식이다.
    if style.flex_grow.unwrap_or(0.0) > 0.0 {
        return None;
    }

    // 구분선: `border-width`가 곧 두께다 (그려지는 방향과 무관하게 주축 두께).
    if let Element::Divider(_) = el {
        return Some(border_of(&style).unwrap_or(0.0).max(0.0) + margin);
    }
    // 스피너: `width`(또는 `font-size`)만큼 정사각.
    if let Element::Spinner(_) = el {
        let size = match style.width {
            Some(Len::Px(v)) => v,
            _ => style.font_size.unwrap_or(BASE_FONT_SIZE),
        };
        return Some(size + margin);
    }
    // 진행 막대: 폭은 채우고, 높이만 선언/기본값으로 예약한다.
    if let Element::Progress(_) = el {
        if horizontal {
            return None;
        }
        let h = match style.height {
            Some(Len::Px(v)) => v,
            _ => style.min_height.unwrap_or(8.0),
        };
        return Some(h + margin);
    }

    // `justify`가 있는 컨테이너는 남는 공간을 받아야 정렬이 의미를 갖는다 (버그 2).
    // 가로 주축에서만 자동 확장한다 — 세로는 남은 높이가 패널 전체라 위험하다.
    if horizontal && matches!(el, Element::Col(_) | Element::Row(_)) && style.justify.is_some() {
        return None;
    }

    let text = |text: &str, bold: bool| text_extent(ui, text, &style, bold, horizontal);
    let content: Option<f32> = match el {
        Element::Text(TextEl { text: t, .. })
        | Element::Td(TdEl { text: t, .. })
        | Element::Banner(BannerEl { text: t, .. }) => Some(text(t, false)),
        Element::Strong(StrongEl { text: t, .. }) | Element::Th(ThEl { text: t, .. }) => {
            Some(text(t, true))
        }
        Element::Button(ButtonEl { text: t, .. }) => Some(text(t, false)),
        Element::Tab(TabEl { text: t, .. }) => Some(text(t, false)),
        Element::Check(CheckEl { label, .. }) => {
            let icon = if horizontal {
                ui.spacing().icon_width + ui.spacing().icon_spacing
            } else {
                0.0
            };
            Some(text(label, false) + icon)
        }
        // 입력은 **폭이 fill**(가변)이고 높이는 글자 높이다 — 세로에서 늘어나면 안 된다.
        Element::Input(_) | Element::TextArea(_) => {
            if horizontal {
                None
            } else {
                Some(text(" ", false))
            }
        }
        // 컨테이너: 부모의 주축이 자기 주축과 같으면 자식 합, 다르면 자식 최댓값.
        Element::Col(ColEl { children, .. }) | Element::Row(RowEl { children, .. }) => {
            let container_horizontal = matches!(el, Element::Row(_));
            let gap = if container_horizontal {
                style.column_gap()
            } else {
                style.row_gap()
            }
            .unwrap_or(0.0);
            if container_horizontal == horizontal {
                let mut total = 0.0;
                for c in children {
                    total += intrinsic_main(walk, ui, c, horizontal).unwrap_or(0.0);
                }
                total += gap * children.len().saturating_sub(1) as f32;
                Some(total)
            } else {
                let mut max = 0.0f32;
                for c in children {
                    if let Some(v) = intrinsic_main(walk, ui, c, horizontal) {
                        max = max.max(v);
                    }
                }
                Some(max)
            }
        }
        Element::Fragment(FragmentEl { children }) => {
            let mut total = 0.0;
            for c in children {
                total += intrinsic_main(walk, ui, c, horizontal).unwrap_or(0.0);
            }
            Some(total)
        }
        // 자리 없는 것들(0) — 오버레이/그림자는 레이아웃을 차지하지 않는다
        _ => Some(0.0),
    };
    content.map(finish)
}

/// 가변 자식(`fill` / `justify` 컨테이너 / 입력)에게 나눠줄 주축 크기와,
/// 주축에 **남는 공간**(정렬용)을 계산한다.
///
/// 뒤 형제의 몫을 **먼저 예약**한 뒤 남은 공간을 가변 자식 수로 나눈다.
/// 그래서 `width: fill`이 뒤 형제를 컨테이너 밖으로 밀어내지 않는다.
/// 가변 자식이 하나라도 있으면 남는 공간은 그쪽이 먹으므로 정렬용 여유는 0이다.
fn child_budgets(
    walk: &Walk<'_>,
    ui: &egui::Ui,
    children: &[Element],
    horizontal: bool,
    gap: f32,
) -> (Vec<Option<f32>>, f32) {
    let available = if horizontal {
        ui.available_width()
    } else {
        ui.available_height()
    };
    if !available.is_finite() || children.is_empty() {
        return (vec![None; children.len()], 0.0);
    }
    let mut fixed_total = 0.0f32;
    let mut intrinsics = Vec::with_capacity(children.len());
    for c in children {
        let v = intrinsic_main(walk, ui, c, horizontal);
        match v {
            Some(v) => fixed_total += v,
            None => {
                // 가변 자식도 마진은 고정 오버헤드다 — 예산에서 미리 뺀다.
                fixed_total += margin_main(&resolve_style(walk, c), horizontal);
            }
        }
        intrinsics.push(v);
    }
    let gap_total = gap.max(0.0) * children.len().saturating_sub(1) as f32;
    let remaining = (available - fixed_total - gap_total).max(0.0);

    let mut budgets: Vec<Option<f32>> = intrinsics.iter().map(|_| None).collect();
    let mut open: Vec<usize> = intrinsics
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.is_none().then_some(i))
        .collect();
    let mut free = remaining;
    // `flex-grow` 가중치 — 선언이 없으면 1(균등), `fill`과 같은 기본.
    let weight = |i: usize| -> f32 {
        resolve_style(walk, &children[i])
            .flex_grow
            .unwrap_or(1.0)
            .max(0.0)
    };
    // 가변 자식에게 남은 공간을 가중 분배하되, `max-*` 상한이나 `min-*` 하한에
    // 닿은 자식은 확정하고 남은 공간을 다시 나눈다 — 예전에는 균등 분배만 해서
    // `height: fill; max-height: 40` 형제가 남긴 공간을 아무도 못 먹었다.
    while !open.is_empty() {
        let total_w: f32 = open.iter().map(|&i| weight(i)).sum();
        let share_of = |i: usize, free: f32| -> f32 {
            if total_w > 0.0 {
                free * weight(i) / total_w
            } else {
                free / open.len() as f32
            }
        };
        let mut capped: Vec<usize> = Vec::new();
        let mut stay: Vec<usize> = Vec::new();
        for &i in &open {
            let s = resolve_style(walk, &children[i]);
            let lo = axis_min(&s, horizontal).unwrap_or(0.0);
            let hi = axis_max(&s, horizontal).unwrap_or(f32::INFINITY);
            let share = share_of(i, free);
            if share > hi + 0.01 || share < lo - 0.01 {
                capped.push(i);
            } else {
                stay.push(i);
            }
        }
        if capped.is_empty() {
            for &i in &open {
                budgets[i] = Some(share_of(i, free));
            }
            break;
        }
        for &i in &capped {
            let s = resolve_style(walk, &children[i]);
            let lo = axis_min(&s, horizontal).unwrap_or(0.0);
            let hi = axis_max(&s, horizontal).unwrap_or(f32::INFINITY);
            let share = share_of(i, free);
            let give = if share > hi + 0.01 { hi } else { lo };
            budgets[i] = Some(give);
            free = (free - give).max(0.0);
        }
        open = stay;
    }
    // 분배하고 남은 공간 — `justify`가 쓸 수 있다 (가변이 상한에 묶인 경우에도).
    let used: f32 = budgets.iter().flatten().sum();
    let extra = (remaining - used).max(0.0);
    (budgets, extra)
}

/// 한 축의 `min-*` (가로면 `min-width`).
fn axis_min(style: &ResolvedStyle, horizontal: bool) -> Option<f32> {
    if horizontal {
        style.min_width
    } else {
        style.min_height
    }
}

/// 한 축의 `max-*` (가로면 `max-width`).
fn axis_max(style: &ResolvedStyle, horizontal: bool) -> Option<f32> {
    if horizontal {
        style.max_width
    } else {
        style.max_height
    }
}

/// 위젯 최소 크기 (버튼) — `width`/`height`(또는 `min-*`)가 있을 때만.
fn min_size(style: &ResolvedStyle) -> Option<egui::Vec2> {
    let w = match style.width {
        Some(Len::Px(v)) => v,
        _ => style.min_width.unwrap_or(0.0),
    };
    let h = match style.height {
        Some(Len::Px(v)) => v,
        _ => style.min_height.unwrap_or(0.0),
    };
    if w > 0.0 || h > 0.0 {
        Some(egui::vec2(w, h))
    } else {
        None
    }
}

/// 버튼 계열 위젯의 크기 — 선언한 px + `fill`(주축 예산 / 교차축 가용)까지.
///
/// `Button`/`Tab`은 컨테이너가 아니라서 [`apply_size`]를 타지 않는다. 그래서
/// `width: fill`이 무시됐다 — 예산을 `min_size`로 옮겨 채운다.
fn widget_size(ui: &egui::Ui, style: &ResolvedStyle, budget: Option<f32>) -> Option<egui::Vec2> {
    let mut size = min_size(style).unwrap_or(egui::Vec2::ZERO);
    let horizontal = ui.layout().main_dir().is_horizontal();
    // `flex-grow`는 부모 주축의 남는 공간을 받는다 (`width`/`height: fill`과 같다).
    let grow = style.flex_grow.unwrap_or(0.0) > 0.0;
    if style.width == Some(Len::Fill) || (grow && horizontal) {
        let v = if horizontal {
            budget
        } else {
            Some(ui.available_width())
        };
        if let Some(v) = v.filter(|v| v.is_finite()) {
            size.x = size.x.max(v);
        }
    }
    if style.height == Some(Len::Fill) || (grow && !horizontal) {
        let v = if horizontal {
            Some(ui.available_height())
        } else {
            budget
        };
        if let Some(v) = v.filter(|v| v.is_finite()) {
            size.y = size.y.max(v);
        }
    }
    if size.x > 0.0 || size.y > 0.0 {
        Some(egui::vec2(
            clamp_axis(size.x, style.min_width, style.max_width),
            clamp_axis(size.y, style.min_height, style.max_height),
        ))
    } else {
        None
    }
}

/// CSS `padding`을 egui 버튼의 내부 여백으로 옮겨 실행한다 (0.7.4 — 리포트 버그 12).
///
/// egui는 `Button`/`SelectableLabel`의 패딩을 `spacing.button_padding`에서
/// 읽는다. 그 값은 가로·세로만 표현하므로 `padding: 8`(네 방향)과
/// `padding: 8 16`(상하/좌우)까지 반영된다. 예전에는 `Button`/`Tab`이
/// `padding`을 아예 무시해 rect가 변하지 않았다.
fn with_button_padding<R>(
    ui: &mut egui::Ui,
    style: &ResolvedStyle,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let Some(padding) = padding_of(style) else {
        return f(ui);
    };
    let saved = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(padding.left, padding.top);
    let out = f(ui);
    ui.spacing_mut().button_padding = saved;
    out
}

/// `cursor`를 응답에 적용한다.
///
/// `opacity`/`visibility: hidden`은 여기서 다루지 않는다 — 그리기 **전에**
/// 어댑터가 노드 스코프에 걸어야 한다(egui의 투명도는 이후에 추가되는 도형에만
/// 적용되므로, 나중에 걸면 그 노드는 그대로 보이고 **뒤 형제가 투명해진다**).
fn decorate(resp: egui::Response, style: &ResolvedStyle) -> egui::Response {
    match style.cursor {
        Some(c) if interactive(style) => resp.on_hover_cursor(cursor_of(c)),
        _ => resp,
    }
}

/// 글자색 — 선언한 `color`가 없으면 팔레트의 `text` (상속의 뿌리).
fn text_color(style: &ResolvedStyle, palette: &Palette) -> egui::Color32 {
    style
        .color
        .map_or_else(|| color32(palette.get(Token::Text)), color32)
}

/// 노드 하나를 그린 결과 (다음 프레임 hover/focus 판정용).
#[derive(Clone, Copy, Debug)]
struct Drawn {
    rect: egui::Rect,
    focused: bool,
}

impl Drawn {
    /// 응답에서 (소유권 없이).
    fn of(resp: &egui::Response) -> Self {
        Drawn {
            rect: resp.rect,
            focused: resp.has_focus(),
        }
    }
}

impl From<egui::Response> for Drawn {
    fn from(resp: egui::Response) -> Self {
        Drawn {
            rect: resp.rect,
            focused: resp.has_focus(),
        }
    }
}

/// `<Banner kind="…">`의 기본 색 토큰.
fn banner_token(kind: &str) -> Token {
    match kind {
        "error" => Token::Error,
        "warn" | "warning" => Token::Warn,
        "success" | "ok" => Token::Success,
        "info" => Token::Info,
        _ => Token::Primary,
    }
}

/// `:disabled` 상태 — v0.7부터 `Widget::is_disabled`가 dispatch한다.
fn is_disabled(el: &Element) -> bool {
    el.is_disabled()
}

/// 노드 하나를 그리기 전에 거는 래퍼 — `opacity`/`visibility`, `scale`, `z-index`.
///
/// - `opacity`는 그리기 **전에** 노드 스코프에 건다 (egui는 이후 도형에만 적용한다).
/// - `scale`은 노드 좌상단 기준 `with_visual_transform`으로 근사한다. egui의
///   `TSTransform`에는 회전이 없어 `rotate`는 여기서 반영할 수 없다.
/// - `z-index > 0`은 전경 레이어로 올린다 (egui `Order` 근사).
fn scoped_node<R>(
    ui: &mut egui::Ui,
    style: &ResolvedStyle,
    id: egui::Id,
    opacity: Option<f32>,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let paint = |ui: &mut egui::Ui| match opacity {
        Some(o) => {
            ui.scope(|ui| {
                ui.set_opacity(o);
                f(ui)
            })
            .inner
        }
        None => f(ui),
    };
    let scale = style
        .scale
        .filter(|s| *s > 0.0 && (*s - 1.0).abs() > f32::EPSILON);
    let transformed = |ui: &mut egui::Ui| match scale {
        Some(s) => {
            let pivot = ui.next_widget_position();
            let t = egui::emath::TSTransform::from_translation(pivot.to_vec2() * (1.0 - s))
                * egui::emath::TSTransform::from_scaling(s);
            ui.with_visual_transform(t, paint).inner
        }
        None => paint(ui),
    };
    if style.z_index.is_some_and(|z| z > 0) {
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new(("elm-z", id)));
        ui.scope_builder(egui::UiBuilder::new().layer_id(layer), transformed)
            .inner
    } else {
        transformed(ui)
    }
}

/// 노드 하나를 그린다 — 스타일 해석(조상·상속·상태) → egui 반영 → 자식 재귀.
fn render_el<'a>(walk: &mut Walk<'a>, ui: &mut egui::Ui, el: &'a Element, arena: &mut Arena) {
    let palette = walk.palette;
    let id = walk.key();
    let state = read_state(ui.ctx(), id, is_disabled(el));
    let mut style = el.resolved_style_in(&walk.ancestors, state, walk.parent_style(), palette);
    walk.node += 1;

    // `display: none` — 자리도 차지하지 않는다
    // (단 **해석된 스타일은 기록한다** — `pass.styles`로 CSS가 파싱됐는지 확인할 수 있다.)
    if style.is_display_none() {
        if !style.is_empty() {
            walk.pass.styles.push((el.tag(), style));
        }
        return;
    }
    // `<Banner>`는 을 안 정하면 종류별 기본색을 쓴다
    if let Element::Banner(BannerEl { kind, .. }) = el {
        if style.color.is_none() {
            style.color = Some(palette.get(banner_token(kind)));
        }
    }
    if !style.is_empty() {
        walk.pass.styles.push((el.tag(), style));
    }

    walk.ancestors.insert(0, el);
    walk.inherited.push(style);

    // 응답을 내지 않는 태그(`Raw`/`Fragment`)는 상태를 기록하지 않는다.
    //
    // `opacity`/`visibility: hidden`은 **그리기 전에** 노드 스코프에 건다 —
    // egui의 투명도는 이후에 추가되는 도형에만 적용되므로, 나중에 걸면 그 노드는
    // 그대로 보이고 뒤 형제가 투명해진다.
    let opacity = if style.hidden == Some(true) {
        Some(0.0)
    } else {
        style.opacity
    };
    let drawn = scoped_node(ui, &style, id, opacity, |ui| {
        draw_body(walk, ui, el, arena, style, palette)
    });

    walk.inherited.pop();
    walk.ancestors.remove(0);
    write_state(ui.ctx(), id, drawn.rect, drawn.focused);
}

/// `Frame`을 쓰지 않는 리프 위젯인가 — 이들만 `margin` 프레임이 필요하다.
///
/// `Col`/`Row`/`Input`/`TextArea`는 [`frame_of`]가 이미 `outer_margin`을 적용한다.
fn needs_margin_frame(el: &Element) -> bool {
    matches!(
        el,
        Element::Text(_)
            | Element::Strong(_)
            | Element::Banner(_)
            | Element::Spinner(_)
            | Element::Divider(_)
            | Element::Progress(_)
            | Element::Button(_)
            | Element::Tab(_)
            | Element::Th(_)
            | Element::Td(_)
            | Element::Check(_)
    )
}

/// 노드 하나를 그린다 — 스타일 해석·투명도·조상 스택은 `render_el`이 끝냈다.
///
/// `Frame`을 쓰지 않는 리프 위젯의 `margin`은 바깥 여백 프레임으로 적용한다
/// (예전에는 컨테이너의 `margin`만 먹고 Button 등의 `margin`은 무시됐다 — 버그 B5).
fn draw_body<'a>(
    walk: &mut Walk<'a>,
    ui: &mut egui::Ui,
    el: &'a Element,
    arena: &mut Arena,
    style: ResolvedStyle,
    palette: &Palette,
) -> Drawn {
    match margin_of(&style) {
        Some(outer) if needs_margin_frame(el) => {
            egui::Frame::NONE
                .outer_margin(margin(outer))
                .show(ui, |ui| {
                    draw_body_inner(walk, ui, el, arena, style, palette)
                })
                .inner
        }
        _ => draw_body_inner(walk, ui, el, arena, style, palette),
    }
}

/// `draw_body`의 본체 — 마진 프레임 결정 뒤에 불린다.
fn draw_body_inner<'a>(
    walk: &mut Walk<'a>,
    ui: &mut egui::Ui,
    el: &'a Element,
    arena: &mut Arena,
    style: ResolvedStyle,
    palette: &Palette,
) -> Drawn {
    let mut drawn = Drawn {
        rect: egui::Rect::ZERO,
        focused: false,
    };
    match el {
        Element::Text(TextEl { text, .. }) => {
            let resp = text_widget(ui, text, &style, false);

            drawn = Drawn::from(decorate(resp, &style));
        }
        Element::Strong(StrongEl { text, .. }) => {
            let resp = text_widget(ui, text, &style, true);

            drawn = Drawn::from(decorate(resp, &style));
        }
        Element::Banner(BannerEl { text, .. }) => {
            let resp = text_widget(ui, text, &style, false);

            drawn = Drawn::from(decorate(resp, &style));
        }
        Element::Spinner(SpinnerEl { .. }) => {
            let mut spinner = egui::Spinner::new();
            let size = match style.width {
                Some(Len::Px(v)) => Some(v),
                _ => style.font_size,
            };
            if let Some(size) = size {
                spinner = spinner.size(size);
            }
            if let Some(c) = style.fill {
                spinner = spinner.color(color32(c));
            }
            let resp = ui.add(spinner);

            drawn = Drawn::from(decorate(resp, &style));
        }
        Element::Divider(DividerEl { .. }) => {
            // 리셋 후에는 선언한 테두리만 보인다 (예전에는 egui 기본 separator)
            drawn = Drawn::from(decorate(divider(ui, &style, palette), &style));
        }
        Element::Progress(ProgressEl { value, .. }) => {
            let mut bar = egui::ProgressBar::new(*value as f32);
            if let Some(c) = style.fill {
                bar = bar.fill(color32(c));
            }
            let height = match style.height {
                Some(Len::Px(v)) => Some(v),
                _ => style.min_height,
            };
            if let Some(h) = height {
                bar = bar.desired_height(h);
            }
            if let Some(r) = style.radius {
                bar = bar.corner_radius(radius(r));
            }
            let resp = ui.add(bar);

            drawn = Drawn::from(decorate(resp, &style));
        }
        Element::Col(ColEl {
            children, on_click, ..
        }) => {
            drawn = container(walk, ui, arena, &style, children, true, on_click.as_deref());
        }
        Element::Row(RowEl {
            children, on_click, ..
        }) => {
            drawn = container(
                walk,
                ui,
                arena,
                &style,
                children,
                false,
                on_click.as_deref(),
            );
        }
        Element::Button(ButtonEl {
            text,
            disabled,
            on_click,
            ..
        }) => {
            let mut button = egui::Button::new(rich(text, &style, false));
            if let Some(bg) = style.bg {
                button = button.fill(color32(bg));
            }
            if let Some(r) = style.radius {
                button = button.corner_radius(radius(r));
            }
            if let Some(size) = widget_size(ui, &style, walk.main_budget) {
                button = button.min_size(size);
            }
            if let Some(width) = border_of(&style) {
                let c = style
                    .border_color
                    .unwrap_or_else(|| palette.get(Token::Border));
                button = button.stroke(egui::Stroke::new(width, color32(c)));
            }
            // `padding`을 egui의 버튼 내부 여백으로 옮긴다 (0.7.4 — 버그 12).
            let resp = with_button_padding(ui, &style, |ui| ui.add_enabled(!disabled, button));

            let resp = decorate(resp, &style);
            if resp.clicked() && interactive(&style) {
                if let Some(h) = on_click {
                    h(arena);
                }
            }
            drawn = Drawn::of(&resp);
            walk.pass.buttons.push((text.clone(), resp));
        }
        Element::Tab(TabEl {
            text,
            active,
            on_click,
            ..
        }) => {
            // `selectable_label`은 크기를 받지 못하므로 `Button::selectable`을
            // 직접 써서 `padding`·`height`를 반영한다 (0.7.4 — 버그 12).
            let mut button = egui::Button::selectable(*active, rich(text, &style, false));
            if let Some(bg) = style.bg {
                button = button.fill(color32(bg));
            }
            if let Some(r) = style.radius {
                button = button.corner_radius(radius(r));
            }
            if let Some(size) = widget_size(ui, &style, walk.main_budget) {
                button = button.min_size(size);
            }
            if let Some(width) = border_of(&style) {
                let c = style
                    .border_color
                    .unwrap_or_else(|| palette.get(Token::Border));
                button = button.stroke(egui::Stroke::new(width, color32(c)));
            }
            let resp = with_button_padding(ui, &style, |ui| ui.add(button));
            let resp = decorate(resp, &style);
            if resp.clicked() && interactive(&style) {
                if let Some(h) = on_click {
                    h(arena);
                }
            }
            drawn = Drawn::of(&resp);
            walk.pass.buttons.push((text.clone(), resp));
        }
        Element::Th(ThEl { text, on_click, .. }) => {
            let mut button = egui::Button::new(rich(text, &style, true));
            if let Some(bg) = style.bg {
                button = button.fill(color32(bg));
            }
            if let Some(r) = style.radius {
                button = button.corner_radius(radius(r));
            }
            if let Some(size) = widget_size(ui, &style, walk.main_budget) {
                button = button.min_size(size);
            }
            let resp = ui.add(button);

            let resp = decorate(resp, &style);
            if resp.clicked() && interactive(&style) {
                if let Some(h) = on_click {
                    h(arena);
                }
            }
            drawn = Drawn::of(&resp);
            walk.pass.buttons.push((text.clone(), resp));
        }
        Element::Td(TdEl { text, .. }) => {
            let resp = text_widget(ui, text, &style, false);

            drawn = Drawn::from(decorate(resp, &style));
        }

        Element::Input(InputEl {
            value,
            on_change,
            on_enter,
            ..
        }) => {
            let mut v = value.clone();
            // 리셋 이후 입력도 `Frame` + 크기 선언을 따른다 —
            // `bg`/`border-*`/`radius`/`padding`/`width`가 (드디어) 먹는다.
            let outer = frame_of(&style, palette).show(ui, |ui| {
                apply_size(ui, &style, walk.main_budget);
                let mut te = egui::TextEdit::singleline(&mut v)
                    .margin(egui::Margin::ZERO) // egui 기본 여백도 리셋
                    .text_color(text_color(&style, palette));
                let wanted = match style.width {
                    Some(Len::Px(v)) => v,
                    _ => ui.available_width(),
                };
                if wanted.is_finite() {
                    te = te.desired_width(wanted);
                }
                ui.add(te)
            });
            let resp = outer.inner;
            if resp.changed() {
                if let Some(h) = on_change {
                    h(arena, v.clone());
                }
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(h) = on_enter {
                    h(arena, v);
                }
            }
            let rect = outer.response.rect;
            decorate(outer.response, &style);
            drawn = Drawn {
                rect,
                focused: resp.has_focus(),
            };
        }
        Element::TextArea(TextAreaEl {
            value,
            on_change,
            on_enter,
            ..
        }) => {
            let mut v = value.clone();
            let outer = frame_of(&style, palette).show(ui, |ui| {
                apply_size(ui, &style, walk.main_budget);
                let mut te = egui::TextEdit::multiline(&mut v)
                    .margin(egui::Margin::ZERO)
                    .text_color(text_color(&style, palette));
                let wanted = match style.width {
                    Some(Len::Px(v)) => v,
                    _ => ui.available_width(),
                };
                if wanted.is_finite() {
                    te = te.desired_width(wanted);
                }
                ui.add(te)
            });
            let resp = outer.inner;
            if resp.changed() {
                if let Some(h) = on_change {
                    h(arena, v.clone());
                }
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(h) = on_enter {
                    h(arena, v);
                }
            }
            let rect = outer.response.rect;
            decorate(outer.response, &style);
            drawn = Drawn {
                rect,
                focused: resp.has_focus(),
            };
        }
        Element::Check(CheckEl {
            checked,
            label,
            on_change,
            ..
        }) => {
            let mut value = *checked;
            let resp = ui.checkbox(&mut value, rich(label, &style, false));
            if resp.changed() && interactive(&style) {
                if let Some(h) = on_change {
                    h(arena, value);
                }
            }
            let resp = decorate(resp, &style);
            drawn = Drawn::of(&resp);
            walk.pass.checks.push((label.clone(), resp));
        }
        Element::Modal(ModalEl {
            title,
            on_close,
            children,
            ..
        }) => {
            let mut open = true;
            let heading = if title.is_empty() {
                "modal"
            } else {
                title.as_str()
            };
            egui::Window::new(heading)
                .frame(frame_of(&style, palette))
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    apply_size(ui, &style, None);
                    for c in children {
                        render_el(walk, ui, c, arena);
                    }
                });
            if !open {
                if let Some(h) = on_close {
                    h(arena);
                }
            }
        }
        Element::Raw(RawEl { widget, .. }) => {
            // 유일한 탈출구 (사양서 7.3): 어댑터 핸들을 그대로 넘긴다
            widget(ui);
        }
        Element::Fragment(FragmentEl { children }) => {
            // 레이아웃 없는 음 — 순서대로 그린다
            for c in children {
                render_el(walk, ui, c, arena);
            }
        }
    }

    drawn
}

/// `Col`/`Row` 공통 — `Frame` + 간격 + 레이아웃 + 클릭.
fn container<'a>(
    walk: &mut Walk<'a>,
    ui: &mut egui::Ui,
    arena: &mut Arena,
    style: &ResolvedStyle,
    children: &'a [Element],
    vertical: bool,
    on_click: Option<&dyn Fn(&mut Arena)>,
) -> Drawn {
    let palette = walk.palette;
    // `flex-direction`이 축을 뒤집는다 (`Row`에 `column`, `Col`에 `row`).
    let vertical = match style.direction {
        Some(Direction::Column) => true,
        Some(Direction::Row) => false,
        None => vertical,
    };
    let horizontal = !vertical;
    let gap = if vertical {
        style.row_gap()
    } else {
        style.column_gap()
    };
    let inner = frame_of(style, palette).show(ui, |ui| {
        // `overflow: hidden/scroll/auto` — 넘치는 자식을 컨테이너 안으로 자른다.
        if clips_children(style) {
            ui.set_clip_rect(ui.max_rect());
        }
        let mut render_children = |walk: &mut Walk<'a>, ui: &mut egui::Ui| {
            if let Some(gap) = gap {
                if vertical {
                    ui.spacing_mut().item_spacing.y = gap;
                } else {
                    ui.spacing_mut().item_spacing.x = gap;
                    // `wrap` 줄바꿈 뒤 줄 간격도 gap을 따른다 (버그 B7 — 예전에는
                    // 가로 간격만 설정해 줄 사이가 0이었다).
                    if style.wrap == Some(true) {
                        ui.spacing_mut().item_spacing.y = gap;
                    }
                }
            }
            apply_size(ui, style, walk.main_budget);
            // 뒤 형제의 몫을 예약한 주축 예산 + 주축에 남는 공간(정렬용)
            let (budgets, extra) =
                child_budgets(walk, ui, children, horizontal, gap.unwrap_or(0.0));
            ui.with_layout(layout_of(style, vertical), |ui| {
                // `justify: center/end` — 남는 공간을 **앞에** 넣는다.
                // (egui의 `Layout::main_align`은 자식 배치에 쓰이지 않아 0.7.4에는 무효였다.)
                let lead = match style.justify {
                    Some(StyleAlign::Center) => extra / 2.0,
                    Some(StyleAlign::End) => extra,
                    _ => 0.0,
                };
                // egui는 앞 여백 **뒤에** item_spacing(선언한 gap)을 한 번 더 넣는다.
                // 고정 오프셋을 그대로 쓰면 실제 위치가 gap만큼 밀리므로(실측 200→204)
                // 프레임워크가 더한 변화량을 빼서 보정한다.
                let spacing = if horizontal {
                    ui.spacing().item_spacing.x
                } else {
                    ui.spacing().item_spacing.y
                };
                let lead = (lead - spacing).max(0.0);
                if lead > 0.0 {
                    let space = if horizontal {
                        egui::vec2(lead, 0.0)
                    } else {
                        egui::vec2(0.0, lead)
                    };
                    ui.allocate_space(space);
                }
                for (child, budget) in children.iter().zip(budgets) {
                    let child_style = resolve_style(walk, child);
                    let saved = walk.main_budget;
                    walk.main_budget = budget;
                    match child_style.align_self {
                        // `align-self` — 이 자식만 교차축 정렬을 바꾼다. egui의 교차축
                        // 정렬은 컨테이너 전역이라 자식 스코프로 우회한다.
                        Some(a) => {
                            let cross = align_of(Some(a));
                            let layout = if horizontal {
                                egui::Layout::left_to_right(cross).with_cross_justify(true)
                            } else {
                                egui::Layout::top_down(cross)
                            };
                            ui.scope_builder(egui::UiBuilder::new().layout(layout), |ui| {
                                render_el(walk, ui, child, arena)
                            });
                        }
                        None => render_el(walk, ui, child, arena),
                    }
                    walk.main_budget = saved;
                }
            });
        };
        // `overflow: scroll/auto` — 주축 방향 스크롤 영역을 만든다.
        if scrolls_children(style) {
            let area = if vertical {
                egui::ScrollArea::vertical()
            } else {
                egui::ScrollArea::horizontal()
            };
            area.auto_shrink([false, false])
                .show(ui, |ui| render_children(walk, ui));
        } else {
            render_children(walk, ui);
        }
    });
    if let Some(h) = on_click {
        let resp = ui.interact(inner.response.rect, inner.response.id, egui::Sense::click());
        if resp.clicked() && interactive(style) {
            h(arena);
        }
    }
    Drawn::from(inner.response)
}

/// 구분선 — 리셋 이후에는 **선언한 테두리만** 그린다(`border-width`/`border-color`).
///
/// 리셋 전에는 egui 기본 separator 스타일이 있었고, `border-*`를 선언해도 그대로였다.
/// `border-width`가 없으면 두께 0이라 아무 자리도 차지하지 않는다.
fn divider(ui: &mut egui::Ui, style: &ResolvedStyle, palette: &Palette) -> egui::Response {
    let thickness = border_of(style).unwrap_or(0.0).max(0.0);
    let color = color32(
        style
            .border_color
            .unwrap_or_else(|| palette.get(Token::Border)),
    );
    let horizontal = ui.layout().main_dir().is_horizontal();
    let cross = if horizontal {
        ui.available_height()
    } else {
        ui.available_width()
    };
    let cross = if cross.is_finite() {
        cross.max(0.0)
    } else {
        0.0
    };
    let size = if horizontal {
        egui::vec2(thickness, cross)
    } else {
        egui::vec2(cross, thickness)
    };
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    if thickness > 0.0 {
        let stroke = egui::Stroke::new(thickness, color);
        if horizontal {
            ui.painter().vline(rect.center().x, rect.y_range(), stroke);
        } else {
            ui.painter().hline(rect.x_range(), rect.center().y, stroke);
        }
    }
    resp
}
