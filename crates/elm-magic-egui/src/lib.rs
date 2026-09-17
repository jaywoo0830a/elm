//! egui adapter (사양서 7.2).
//!
//! 어댑터는 라이브러리 내부에 있고, 클라이언트는 egui의 이름조차 모른다.
//! `render`는 `Element` 트리를 egui 위젯으로 그리고, 클릭/입력을 아레나로
//! 전달한다. `<Raw>`는 어댑터 핸들(`&mut egui::Ui`)을 받는 유일한 탈출구다.

use elm_magic::{Arena, Element};

/// 한 패스가 만든 위젯 정보.
pub struct Pass {
    /// 그려진 버튼: (라벨, 응답)
    pub buttons: Vec<(String, egui::Response)>,
    /// 그려진 체크박스: (라벨, 응답)
    pub checks: Vec<(String, egui::Response)>,
}

/// 트리를 egui로 렌더링하고 상호작용을 아레나로 전달한다.
pub fn render(ui: &mut egui::Ui, tree: &Element, arena: &mut Arena) -> Pass {
    let mut pass = Pass { buttons: Vec::new(), checks: Vec::new() };
    render_el(ui, tree, arena, &mut pass);
    pass
}

fn render_el(ui: &mut egui::Ui, el: &Element, arena: &mut Arena, pass: &mut Pass) {
    match el {
        Element::Text { text, .. } => {
            ui.label(text.as_str());
        }
        Element::Strong { text, .. } => {
            ui.strong(text.as_str());
        }
        Element::Banner { kind, text, .. } => {
            let color = match kind.as_str() {
                "error" => egui::Color32::from_rgb(200, 60, 60),
                "warn" | "warning" => egui::Color32::from_rgb(200, 150, 40),
                _ => egui::Color32::from_rgb(70, 130, 200),
            };
            ui.colored_label(color, text.as_str());
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
            let inner = ui.vertical(|ui| {
                for c in children {
                    render_el(ui, c, arena, pass);
                }
            });
            // 클릭 가능한 컨테이너 (`<Row on_click={…}>`)
            if let Some(h) = on_click {
                let resp = ui.interact(inner.response.rect, inner.response.id, egui::Sense::click());
                if resp.clicked() {
                    h(arena);
                }
            }
        }
        Element::Row { children, on_click, .. } => {
            let inner = ui.horizontal(|ui| {
                for c in children {
                    render_el(ui, c, arena, pass);
                }
            });
            if let Some(h) = on_click {
                let resp = ui.interact(inner.response.rect, inner.response.id, egui::Sense::click());
                if resp.clicked() {
                    h(arena);
                }
            }
        }
        Element::Button { text, disabled, on_click, .. } => {
            let resp = ui.add_enabled(!disabled, egui::Button::new(text.as_str()));
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
                    render_el(ui, c, arena, pass);
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
                render_el(ui, c, arena, pass);
            }
        }
    }
}
