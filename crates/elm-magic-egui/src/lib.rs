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
//! (`Input`/`TextArea`/`Raw`는 플랫폼 위에 맡다.)

use elm_magic::style::{
    Align as StyleAlign, Color, Cursor, Edges, Len, Palette, ResolvedStyle, State, Token,
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
    };
    render_el(&mut walk, ui, tree, arena);
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

/// 테이너 레이아웃 — `align`은 교차축(items), `justify`는 주축(content).
fn layout_of(style: &ResolvedStyle, vertical: bool) -> egui::Layout {
    let cross = align_of(style.align);
    let layout = if vertical {
        egui::Layout::top_down(cross)
    } else {
        egui::Layout::left_to_right(cross)
    };
    match style.justify {
        Some(j) => layout.with_main_align(align_of(Some(j))),
        None => layout,
    }
}

/// 배경·여백·모서리·테두리·그림자 → egui `Frame`.
fn frame_of(style: &ResolvedStyle, palette: &Palette) -> egui::Frame {
    let mut frame = egui::Frame::NONE;
    if let Some(bg) = style.bg {
        frame = frame.fill(color32(bg));
    }
    if let Some(padding) = style.padding {
        frame = frame.inner_margin(margin(padding));
    }
    if let Some(outer) = style.margin {
        frame = frame.outer_margin(margin(outer));
    }
    if let Some(r) = style.radius {
        frame = frame.corner_radius(radius(r));
    }
    if let Some(width) = style.border_width {
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
    if style.truncate == Some(true) {
        label = label.truncate();
    }
    ui.add(label)
}

/// 컨테이너/위젯 크기 (`width`/`height`/`min`/`max`).
fn apply_size(ui: &mut egui::Ui, style: &ResolvedStyle) {
    match style.width {
        Some(Len::Px(v)) => ui.set_width(v),
        Some(Len::Fill) => {
            let available = ui.available_width();
            ui.set_width(available);
        }
        _ => {}
    }
    match style.height {
        Some(Len::Px(v)) => ui.set_height(v),
        Some(Len::Fill) => {
            let available = ui.available_height();
            ui.set_height(available);
        }
        _ => {}
    }
    if let Some(v) = style.min_width {
        ui.set_min_width(v);
    }
    if let Some(v) = style.min_height {
        ui.set_min_height(v);
    }
    if let Some(v) = style.max_width {
        ui.set_max_width(v);
    }
    if let Some(v) = style.max_height {
        ui.set_max_height(v);
    }
}

/// 위젯 최소 크기 (버튼) — `width`/`height`(또는 `min-height`)가 있을 때만.
fn min_size(style: &ResolvedStyle) -> Option<egui::Vec2> {
    let w = match style.width {
        Some(Len::Px(v)) => v,
        _ => 0.0,
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
    let Some(padding) = style.padding else {
        return f(ui);
    };
    let saved = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(padding.left, padding.top);
    let out = f(ui);
    ui.spacing_mut().button_padding = saved;
    out
}

/// `cursor` + `opacity` + `visibility`를 UI/응답에 적용한다.
fn decorate(ui: &mut egui::Ui, resp: egui::Response, style: &ResolvedStyle) -> egui::Response {
    if style.hidden == Some(true) {
        // 자리는 차지하되 안 보인다 (`visibility: hidden`)
        ui.set_opacity(0.0);
    } else if let Some(o) = style.opacity {
        ui.set_opacity(o);
    }
    match style.cursor {
        Some(c) => resp.on_hover_cursor(cursor_of(c)),
        None => resp,
    }
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

/// 노드 하나를 그린다 — 스타일 해석(조상·상속·상태) → egui 반영 → 자식 재귀.
fn render_el<'a>(walk: &mut Walk<'a>, ui: &mut egui::Ui, el: &'a Element, arena: &mut Arena) {
    let palette = walk.palette;
    let id = walk.key();
    let state = read_state(ui.ctx(), id, is_disabled(el));
    let mut style = el.resolved_style_in(&walk.ancestors, state, walk.parent_style(), palette);
    walk.node += 1;

    // `display: none` — 자리도 차지하지 않는다
    if style.is_display_none() {
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

    // 응답을 내지 않는 태그(`Raw`/`Fragment`)는 상태를 기록하지 않는다
    let mut drawn = Drawn {
        rect: egui::Rect::ZERO,
        focused: false,
    };
    match el {
        Element::Text(TextEl { text, .. }) => {
            let resp = text_widget(ui, text, &style, false);

            drawn = Drawn::from(decorate(ui, resp, &style));
        }
        Element::Strong(StrongEl { text, .. }) => {
            let resp = text_widget(ui, text, &style, true);

            drawn = Drawn::from(decorate(ui, resp, &style));
        }
        Element::Banner(BannerEl { text, .. }) => {
            let resp = text_widget(ui, text, &style, false);

            drawn = Drawn::from(decorate(ui, resp, &style));
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

            drawn = Drawn::from(decorate(ui, resp, &style));
        }
        Element::Divider(DividerEl { .. }) => {
            let resp = ui.separator();

            drawn = Drawn::from(decorate(ui, resp, &style));
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

            drawn = Drawn::from(decorate(ui, resp, &style));
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
            if let Some(size) = min_size(&style) {
                button = button.min_size(size);
            }
            if let Some(width) = style.border_width {
                let c = style
                    .border_color
                    .unwrap_or_else(|| palette.get(Token::Border));
                button = button.stroke(egui::Stroke::new(width, color32(c)));
            }
            // `padding`을 egui의 버튼 내부 여백으로 옮긴다 (0.7.4 — 버그 12).
            let resp = with_button_padding(ui, &style, |ui| ui.add_enabled(!disabled, button));

            let resp = decorate(ui, resp, &style);
            if resp.clicked() {
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
            if let Some(size) = min_size(&style) {
                button = button.min_size(size);
            }
            if let Some(width) = style.border_width {
                let c = style
                    .border_color
                    .unwrap_or_else(|| palette.get(Token::Border));
                button = button.stroke(egui::Stroke::new(width, color32(c)));
            }
            let resp = with_button_padding(ui, &style, |ui| ui.add(button));
            let resp = decorate(ui, resp, &style);
            if resp.clicked() {
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
            let resp = ui.add(button);

            let resp = decorate(ui, resp, &style);
            if resp.clicked() {
                if let Some(h) = on_click {
                    h(arena);
                }
            }
            drawn = Drawn::of(&resp);
            walk.pass.buttons.push((text.clone(), resp));
        }
        Element::Td(TdEl { text, .. }) => {
            let resp = text_widget(ui, text, &style, false);

            drawn = Drawn::from(decorate(ui, resp, &style));
        }

        Element::Input(InputEl {
            value,
            on_change,
            on_enter,
            ..
        }) => {
            let mut v = value.clone();
            let resp = ui.text_edit_singleline(&mut v);
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
            drawn = Drawn::from(decorate(ui, resp, &style));
        }
        Element::TextArea(TextAreaEl {
            value,
            on_change,
            on_enter,
            ..
        }) => {
            let mut v = value.clone();
            let resp = ui.text_edit_multiline(&mut v);
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
            drawn = Drawn::from(decorate(ui, resp, &style));
        }
        Element::Check(CheckEl {
            checked,
            label,
            on_change,
            ..
        }) => {
            let mut value = *checked;
            let resp = ui.checkbox(&mut value, rich(label, &style, false));
            if resp.changed() {
                if let Some(h) = on_change {
                    h(arena, value);
                }
            }
            let resp = decorate(ui, resp, &style);
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
                    apply_size(ui, &style);
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

    walk.inherited.pop();
    walk.ancestors.remove(0);
    write_state(ui.ctx(), id, drawn.rect, drawn.focused);
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
    let gap = if vertical {
        style.row_gap()
    } else {
        style.column_gap()
    };
    let inner = frame_of(style, palette).show(ui, |ui| {
        if let Some(gap) = gap {
            if vertical {
                ui.spacing_mut().item_spacing.y = gap;
            } else {
                ui.spacing_mut().item_spacing.x = gap;
            }
        }
        apply_size(ui, style);
        ui.with_layout(layout_of(style, vertical), |ui| {
            for child in children {
                render_el(walk, ui, child, arena);
            }
        });
    });
    if let Some(h) = on_click {
        let resp = ui.interact(inner.response.rect, inner.response.id, egui::Sense::click());
        if resp.clicked() {
            h(arena);
        }
    }
    Drawn::from(inner.response)
}
