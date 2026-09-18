//! Element — the pure data view tree.
//!
//! v0.7: `Element`는 이제 **위젯 구조체들의 enum**이고, 그 정의는
//! [`crate::widget`]에 있다. 이 모듈은 코어와 위젯 프로토콜을 잇는
//! 얇은 층이다 — 핸들러 타입, 스타일 해석, `IntoElements`/`IntoElement`,
//! 그리고 콜백 prop.

use std::rc::Rc;

use crate::state::Arena;
use crate::widget::{FragmentEl, Widget};

/// Event handler: receives the state arena.
pub type Handler = Rc<dyn Fn(&mut Arena)>;
/// Event handler with an incoming value (Input events).
pub type ValueHandler = Rc<dyn Fn(&mut Arena, String)>;
/// Event handler with an incoming bool (Check events).
pub type BoolHandler = Rc<dyn Fn(&mut Arena, bool)>;

// `Element`는 `widget` 모듈에 정의된다 — 기존 `crate::element::Element` 경로 호환.
pub use crate::widget::Element;

impl Element {
    /// 이 엘리먼트에 적용될 **최종 스타일** (사양서 6.1~6.3).
    pub fn resolved_style(&self, palette: &crate::style::Palette) -> crate::style::ResolvedStyle {
        crate::style::resolve(self.class(), self.tag(), palette)
    }

    /// 셀렉터 매칭에 쓰는 노드 정보 (태그 + 클래스).
    pub fn node(&self) -> crate::style::Node<'_> {
        crate::style::Node::new(self.tag(), self.class())
    }

    /// **트리 안에서**의 최종 스타일 — 후손/자식 셀렉터·상태·상속까지.
    pub fn resolved_style_in(
        &self,
        ancestors: &[&Element],
        state: crate::style::State,
        inherited: Option<&crate::style::ResolvedStyle>,
        palette: &crate::style::Palette,
    ) -> crate::style::ResolvedStyle {
        let mut path: Vec<crate::style::Node> = Vec::with_capacity(ancestors.len() + 1);
        path.push(self.node());
        for ancestor in ancestors {
            path.push(ancestor.node());
        }
        crate::style::resolve_nodes(&path, state, inherited, palette)
    }

    /// 요소 서브트리의 텍스트 노드들 — `Widget::texts`가 dispatch된다.
    pub fn texts(&self) -> Vec<String> {
        Widget::texts(self)
    }

    /// 서브트리 텍스트를 이어 붙인다 (클릭 가능한 컨테이너 매칭용).
    pub fn subtree_text(&self) -> String {
        Widget::subtree_text(self)
    }

    /// 자식 요소를 갖는 컨테이너면 그 목록 (트리 순회용).
    pub fn children(&self) -> Option<&[Element]> {
        Widget::children(self)
    }

    /// 이 노드의 클래스 목록 (`Widget::class`로 dispatch).
    pub fn class(&self) -> &[String] {
        Widget::class(self)
    }

    /// `css!`의 태그 셀렉터 이름 (`button { … }`). Fragment는 `""`.
    pub fn tag(&self) -> &'static str {
        Widget::tag(self)
    }
}

/// Convert anything element-like into a list of elements.
///
/// `if` / `match` / iterators / `Option` 모두를 `ui!`·`#[view]` 본문의
/// 자식으로 쓰게 하는 계약 — 매크로가 각 분기를 `into_elements(..)`로
/// 감싸므로 분기마다 타입이 달라도 같은 타입이 된다 (사양서 3.4).
pub trait IntoElements {
    fn into_elements(self) -> Vec<Element>;
}

impl IntoElements for Element {
    fn into_elements(self) -> Vec<Element> {
        vec![self]
    }
}

impl<I, T> IntoElements for I
where
    I: IntoIterator<Item = T>,
    T: IntoElements,
{
    fn into_elements(self) -> Vec<Element> {
        self.into_iter()
            .flat_map(IntoElements::into_elements)
            .collect()
    }
}

/// `props.on_select(item.id)` — 컴포넌트가 부모에게 값을 올려보내는 콜백
/// (사양서 3.1의 `on_select: fn(Id)`).
pub struct Callback<T> {
    f: Rc<dyn Fn(&mut Arena, T)>,
}

impl<T> Clone for Callback<T> {
    fn clone(&self) -> Self {
        Callback {
            f: Rc::clone(&self.f),
        }
    }
}

impl<T> Callback<T> {
    pub fn new(f: impl Fn(&mut Arena, T) + 'static) -> Self {
        Callback { f: Rc::new(f) }
    }

    /// 호출: `on_select(item.id)` → `cb.call(arena, item.id)`.
    pub fn call(&self, arena: &mut Arena, value: T) {
        (self.f)(arena, value)
    }
}

impl<T> Default for Callback<T> {
    /// 기본값: 아무 일도 하지 않는 콜백 (prop을 주지 않았을 때).
    fn default() -> Self {
        Callback {
            f: Rc::new(|_, _| {}),
        }
    }
}

/// 뷰 본문의 값 → 단일 `Element`.
pub trait IntoElement {
    fn into_element(self) -> Element;
}

impl IntoElement for Element {
    fn into_element(self) -> Element {
        self
    }
}

impl IntoElement for Vec<Element> {
    fn into_element(self) -> Element {
        Element::Fragment(FragmentEl { children: self })
    }
}
