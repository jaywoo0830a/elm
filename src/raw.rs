//! `<Raw>` — the platform escape hatch (사양서 7.3).
//!
//! `<Raw>|ui: &mut PlatformType| { ... }</Raw>` stores a closure opaque to
//! elm-magic. Headless testing ignores it; the platform adapter walks the
//! tree and invokes each Raw widget with its own handle (`&mut egui::Ui`).
//!
//! v0.7: 트리 순회가 `Widget::raw_fn` / `Widget::children` **프로토콜**로
//! 바뀌었다 — variant 매칭이 없다.

use std::any::Any;
use std::rc::Rc;

use crate::element::Element;
use crate::widget::Widget;

/// An opaque platform widget closure.
pub type RawFn = Rc<dyn Fn(&mut dyn Any)>;

/// Invoke a user closure with a downcast platform payload.
pub fn call_raw<T: Any, F: FnOnce(&mut T)>(payload: &mut dyn Any, f: F) {
    let t = payload
        .downcast_mut::<T>()
        .expect("elm-magic <Raw>: platform payload type mismatch");
    f(t);
}

/// Walk the tree and invoke every `<Raw>` widget with `payload`.
pub fn invoke(element: &Element, payload: &mut dyn Any) {
    if let Some(widget) = element.raw_fn() {
        widget(payload);
        return;
    }
    if let Some(children) = element.children() {
        for c in children {
            invoke(c, payload);
        }
    }
}
