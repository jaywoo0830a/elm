//! egui 어댑터 (사양서 7.2).
//!
//! 어댑터는 라이브러리 내부에 있고, 클라이언트는 egui의 이름조차 모른다.
//! `render`는 `Element` 트리를 egui 위젯으로 그리고, 클릭/입력을 아레나로
//! 전달한다. `<Raw>`는 어댑터 핸들(`&mut egui::Ui`)을 받는 유일한 탈출구다.
//!
//! ## v0.6 — `css!` 스타일 적용 (사양서 6.1)
//!
//! 해석은 코어가 끝낸다(`Element::resolved_style` → `ResolvedStyle`).
//! 여기서는 그 값을 egui 값으로 **옮기기만** 한다 — 덕분에 스타일 계약
//! 대부분은 egui 없이 `tests/style.rs`에서 검증된다.
//!
//! 스타일이 적용되는 태그는 6개다: `Col` `Row` `Text` `Strong` `Button` `Banner`.
//! 나머지 태그의 `class`는 아직 무시된다.

use elm_magic::style::{Color, Edges, Palette, ResolvedStyle, Token};
use elm_magic::{Arena, Element};

/// 한 패스가 만든 위젯 정보.
pub struct Pass {
    /// 그려진 버튼: (라벨, 응답)
    pub buttons: Vec<(String, egui::Response)>,
    /// 그려진 체크박스: (라벨, 응답)
    pub checks: Vec<(String, egui::Response)>,
    /// **스타일이 실제로 적용된** 엘리먼트: (태그, 확정 스타일).
    ///
    /// egui 내부를 들여다보지 않고도 스타일이 전달됐는지 검증할 수 있다.
    pub styles: Vec<(&'static str, ResolvedStyle)>,
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
    let mut pass = Pass { buttons: Vec::new(), checks: Vec::new(), styles: Vec::new() };
    render_el(ui, tree, arena, &mut pass, palette);
    pass
}

// ── 스타일 → egui ────────────────────────────────────────────

fn color32(color: Color) -> egui::Color32 {
    egui::Color32::from_rgb(color.r, color.g, color.b)
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
    egui::CornerRadius::same(value.max(0.0) as u8)
}

/// 배경·패딩·마진·모서리 → egui `Frame`.
fn frame_of(style: &ResolvedStyle) -> egui::Frame {
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
    frame
}

/// 글자 스타일 → `RichText` (`default_bold`는 태그 기본값).
fn rich(text: &str, style: &ResolvedStyle, default_bold: bool) -> egui::RichText {
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
    if style.strike.unwrap_or(false) {
        out = out.strikethrough();
    }
    if style.underline.unwrap_or(false) {
        out = out.underline();
    }
    out
}

/// 스타일이 적용된 엘리먼트를 기록한다 (`Pass::styles`).
fn record(pass: &mut Pass, tag: &'static str, style: &ResolvedStyle) {
    if !style.is_empty() {
        pass.styles.push((tag, *style));
    }
}

/// `<Banner kind="…">`의 기본 색 토큰.
fn banner_token(kind: &str) -> Token {
    match kind {
        "error" => Token::Error,
        "warn" | "warning" => Token::Warn,
        _ => Token::Primary,
    }
}

fn render_el(
    ui: &mut egui::Ui,
    el: &Element,
    arena: &mut Arena,
    pass: &mut Pass,
    palette: &Palette,
) {
    match el {
        Element::Text { text, .. } => {
            let style = el.resolved_style(palette);
            record(pass, el.tag(), &style);
            ui.label(rich(text, &style, false));
        }
        Element::Strong { text, .. } => {
            let style = el.resolved_style(palette);
            record(pass, el.tag(), &style);
            ui.label(rich(text, &style, true));
        }
        Element::Banner { kind, text, .. } => {
            let mut style = el.resolved_style(palette);
            if style.color.is_none() {
                style.color = Some(palette.get(banner_token(kind)));
            }
            record(pass, el.tag(), &style);
            ui.label(rich(text, &style, false));
        }
        Element::Spinner { .. } => {
            ui.spinner();
        }
        Element::Divider { .. } => {
            ui.separator();
        }
        Element::Progress { value, .. } => {
            ui.add(egui::ProgressBar::new(*value as f32));
        }
        Element::Col { children, on_click, .. } => {
            let style = el.resolved_style(palette);
            record(pass, el.tag(), &style);
            let gap = style.gap;
            let inner = frame_of(&style).show(ui, |ui| {
                if let Some(gap) = gap {
                    ui.spacing_mut().item_spacing.y = gap;
                }
                ui.vertical(|ui| {
                    for c in children {
                        render_el(ui, c, arena, pass, palette);
                    }
                });
            });
            // 클릭 가능한 컨테이너 (`<Col on_click={…}>`)
            if let Some(h) = on_click {
                let resp =
                    ui.interact(inner.response.rect, inner.response.id, egui::Sense::click());
                if resp.clicked() {
                    h(arena);
                }
            }
        }
        Element::Row { children, on_click, .. } => {
            let style = el.resolved_style(palette);
            record(pass, el.tag(), &style);
            let gap = style.gap;
            let inner = frame_of(&style).show(ui, |ui| {
                if let Some(gap) = gap {
                    ui.spacing_mut().item_spacing.x = gap;
                }
                ui.horizontal(|ui| {
                    for c in children {
                        render_el(ui, c, arena, pass, palette);
                    }
                });
            });
            if let Some(h) = on_click {
                let resp =
                    ui.interact(inner.response.rect, inner.response.id, egui::Sense::click());
                if resp.clicked() {
                    h(arena);
                }
            }
        }

Element::Button { text, disabled, on_click, .. } => {
            let style = el.resolved_style(palette);
            record(pass, el.tag(), &style);
            let mut button = egui::Button::new(rich(text, &style, false));
            if let Some(bg) = style.bg {
                button = button.fill(color32(bg));
            }
            if let Some(r) = style.radius {
                button = button.corner_radius(radius(r));
            }
            let resp = ui.add_enabled(!disabled, button);
            if resp.clicked() {
                if let Some(h) = on_click {
                    h(arena);
                }
            }
            pass.buttons.push((text.clone(), resp));
        }
        Element::Tab { text, active, on_click, .. } => {
            let resp = ui.selectable_label(*active, text.as_str());
            if resp.clicked() {
                if let Some(h) = on_click {
                    h(arena);
                }
            }
            pass.buttons.push((text.clone(), resp));
        }
        Element::Th { text, on_click, .. } => {
            let resp = ui.add(egui::Button::new(egui::RichText::new(text.as_str()).strong()));
            if resp.clicked() {
                if let Some(h) = on_click {
                    h(arena);
                }
            }
            pass.buttons.push((text.clone(), resp));
        }
        Element::Td { text, .. } => {
            ui.label(text.as_str());
        }
        Element::Input { value, on_change, on_enter, .. } => {
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
        }
        Element::TextArea { value, on_change, on_enter, .. } => {
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
        }
        Element::Check { checked, label, on_change, .. } => {
            let mut value = *checked;
            let resp = ui.checkbox(&mut value, label.as_str());
            if resp.changed() {
                if let Some(h) = on_change {
                    h(arena, value);
                }
            }
            pass.checks.push((label.clone(), resp));
        }
        Element::Modal { title, on_close, children, .. } => {
            let mut open = true;
            let heading = if title.is_empty() { "modal" } else { title.as_str() };
            egui::Window::new(heading).open(&mut open).show(ui.ctx(), |ui| {
                for c in children {
                    render_el(ui, c, arena, pass, palette);
                }
            });
            if !open {
                if let Some(h) = on_close {
                    h(arena);
                }
            }
        }
        Element::Raw { widget, .. } => {
            // 유일한 탈출구 (사양서 7.3): 어댑터 핸들을 그대로 넘긴다
            widget(ui);
        }
        Element::Fragment { children } => {
            // 레이아웃 없는 묶음 — 순서대로 그린다
            for c in children {
                render_el(ui, c, arena, pass, palette);
            }
        }
    }
}
