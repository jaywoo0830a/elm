//! Element — the pure data view tree.

use std::rc::Rc;

use crate::state::Arena;

/// Event handler: receives the state arena.
pub type Handler = Rc<dyn Fn(&mut Arena)>;
/// Event handler with an incoming value (Input events).
pub type ValueHandler = Rc<dyn Fn(&mut Arena, String)>;

/// The view tree. Pure data — same state in, same tree out.
#[derive(Clone)]
pub enum Element {
    Text {
        text: String,
        class: Vec<String>,
    },
    Col {
        class: Vec<String>,
        children: Vec<Element>,
    },
    Row {
        class: Vec<String>,
        children: Vec<Element>,
    },
    Button {
        text: String,
        class: Vec<String>,
        disabled: bool,
        on_click: Option<Handler>,
    },
    Input {
        value: String,
        class: Vec<String>,
        on_change: Option<ValueHandler>,
        on_enter: Option<ValueHandler>,
    },
    /// `<Raw>|ui: &mut PlatformType| { ... }</Raw>` — the escape hatch
    /// (사양서 7.3). Opaque to headless testing; the platform adapter
    /// invokes it with its own handle (e.g. `&mut egui::Ui`).
    Raw {
        class: Vec<String>,
        widget: crate::raw::RawFn,
    },
}

impl Element {
    pub fn class(&self) -> &[String] {
        match self {
            Element::Text { class, .. }
            | Element::Col { class, .. }
            | Element::Row { class, .. }
            | Element::Button { class, .. }
            | Element::Input { class, .. }
            | Element::Raw { class, .. } => class,
        }
    }
}

/// Convert anything element-like into a list of elements.
///
/// This is what makes `if` / `else if let` / `match` / iterators / `Option`
/// all usable as children inside `ui!` and `#[view]` bodies.
pub trait IntoElements {
    fn into_elements(self) -> Vec<Element>;
}

impl IntoElements for Element {
    fn into_elements(self) -> Vec<Element> {
        vec![self]
    }
}

// Note: iterators of elements are covered by the blanket `Iterator` impl
// below (e.g. `items.map(|t| <Row>...)`). A bare `Vec<Element>` should be
// passed through `.into_iter()`.

impl<I, T> IntoElements for I
where
    I: Iterator<Item = T>,
    T: IntoElements,
{
    fn into_elements(self) -> Vec<Element> {
        self.flat_map(IntoElements::into_elements).collect()
    }
}
