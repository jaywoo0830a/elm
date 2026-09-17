//! Element — the pure data view tree.

use std::rc::Rc;

use crate::state::Arena;

/// Event handler: receives the state arena.
pub type Handler = Rc<dyn Fn(&mut Arena)>;
/// Event handler with an incoming value (Input events).
pub type ValueHandler = Rc<dyn Fn(&mut Arena, String)>;
/// Event handler with an incoming bool (Check events).
pub type BoolHandler = Rc<dyn Fn(&mut Arena, bool)>;

/// The view tree. Pure data — same state in, same tree out.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Element {
    Text {
        text: String,
        class: Vec<String>,
    },
    /// `<Strong>` — 강조 텍스트
    Strong {
        text: String,
        class: Vec<String>,
    },
    Col {
        class: Vec<String>,
        children: Vec<Element>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_click: Option<Handler>,
    },
    Row {
        class: Vec<String>,
        children: Vec<Element>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_click: Option<Handler>,
    },
    Button {
        text: String,
        class: Vec<String>,
        disabled: bool,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_click: Option<Handler>,
    },
    Input {
        value: String,
        class: Vec<String>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_change: Option<ValueHandler>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_enter: Option<ValueHandler>,
    },
    /// `<TextArea>` — 여러 줄 입력 (Input과 동일한 이벤트 계약)
    TextArea {
        value: String,
        class: Vec<String>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_change: Option<ValueHandler>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_enter: Option<ValueHandler>,
    },
    /// `<Check checked={..} on_change={..}>"라벨"</Check>`
    Check {
        checked: bool,
        label: String,
        class: Vec<String>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_change: Option<BoolHandler>,
    },
    /// `<Tab active={..} on_click={..}>"라벨"</Tab>`
    Tab {
        text: String,
        active: bool,
        class: Vec<String>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_click: Option<Handler>,
    },
    /// `<Th on_click={..}>` — 표 헤더 셀
    Th {
        text: String,
        class: Vec<String>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_click: Option<Handler>,
    },
    /// `<Td>` — 표 본문 셀
    Td {
        text: String,
        class: Vec<String>,
    },
    /// `<Banner kind="error">"{e}"</Banner>`
    Banner {
        kind: String,
        text: String,
        class: Vec<String>,
    },
    /// `<Spinner />`
    Spinner {
        class: Vec<String>,
    },
    /// `<Divider />`
    Divider {
        class: Vec<String>,
    },
    /// `<Progress value={0.5} />`
    Progress {
        value: f64,
        class: Vec<String>,
    },
    /// `<Modal on_close={..}>children</Modal>` — 자식을 갖는 빌트인
    Modal {
        title: String,
        class: Vec<String>,
        #[cfg_attr(feature = "serde", serde(skip))]
        on_close: Option<Handler>,
        children: Vec<Element>,
    },
    /// `<Raw>|ui: &mut PlatformType| { ... }</Raw>` — the escape hatch
    /// (사양서 7.3). Opaque to headless testing; the platform adapter
    /// invokes it with its own handle (e.g. `&mut egui::Ui`).
    Raw {
        class: Vec<String>,
        #[cfg_attr(feature = "serde", serde(skip))]
        widget: crate::raw::RawFn,
    },
    /// 뷰 본문/분기가 요소 여러 개를 낼 때의 컨테이너 (`{if ..}`, `{match ..}`).
    /// 레이아웃을 갖지 않는 순수 묶음이다.
    Fragment {
        children: Vec<Element>,
    },
}

impl Element {
    pub fn class(&self) -> &[String] {
        match self {
            Element::Text { class, .. }
            | Element::Strong { class, .. }
            | Element::Col { class, .. }
            | Element::Row { class, .. }
            | Element::Button { class, .. }
            | Element::Input { class, .. }
            | Element::TextArea { class, .. }
            | Element::Check { class, .. }
            | Element::Tab { class, .. }
            | Element::Th { class, .. }
            | Element::Td { class, .. }
            | Element::Banner { class, .. }
            | Element::Spinner { class }
            | Element::Divider { class }
            | Element::Progress { class, .. }
            | Element::Modal { class, .. }
            | Element::Raw { class, .. } => class,
            Element::Fragment { .. } => &[],
        }
    }

    /// 요소 서브트리의 텍스트 노드들 (헤드리스 테스트의 `text()`/`assert_*` 기반).
    pub fn texts(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_texts(&mut out);
        out
    }

    fn collect_texts(&self, out: &mut Vec<String>) {
        match self {
            Element::Text { text, .. }
            | Element::Strong { text, .. }
            | Element::Button { text, .. }
            | Element::Tab { text, .. }
            | Element::Th { text, .. }
            | Element::Td { text, .. }
            | Element::Banner { text, .. } => out.push(text.clone()),
            Element::Input { value, .. } | Element::TextArea { value, .. } => {
                out.push(format!("[input: {}]", value))
            }
            Element::Check { label, .. } => out.push(label.clone()),
            Element::Spinner { .. } => out.push("[spinner]".to_string()),
            Element::Divider { .. } => out.push("[divider]".to_string()),
            Element::Progress { value, .. } => out.push(format!("[progress: {}]", value)),
            // `<Raw>` is ignored headless (사양서 7.3)
            Element::Raw { .. } => {}
            Element::Modal { children, .. } => {
                out.push("[modal]".to_string());
                for c in children {
                    c.collect_texts(out);
                }
            }
            Element::Col { children, .. }
            | Element::Row { children, .. }
            | Element::Fragment { children } => {
                for c in children {
                    c.collect_texts(out);
                }
            }
        }
    }

    /// 요소 서브트리의 텍스트를 이어 붙인다 (클릭 가능한 컨테이너 매칭용).
    pub fn subtree_text(&self) -> String {
        self.texts().join("")
    }

    /// 자식 요소를 갖는 컨테이너면 그 목록을 돌려준다 (트리 순회용).
    pub fn children(&self) -> Option<&[Element]> {
        match self {
            Element::Col { children, .. }
            | Element::Row { children, .. }
            | Element::Modal { children, .. }
            | Element::Fragment { children } => Some(children),
            _ => None,
        }
    }
}

/// Convert anything element-like into a list of elements.
///
/// This is what makes `if` / `else if let` / `match` / iterators / `Option`
/// all usable as children inside `ui!` and `#[view]` bodies — including
/// **분기마다 타입이 다른** 경우(`Element` / `Vec<Element>` / `Option<Element>`):
/// 매크로가 각 분기를 `into_elements(..) -> Vec<Element>`로 감싸기 때문에
/// `if`의 두 분기가 같은 타입이 된다 (사양서 3.4).
pub trait IntoElements {
    fn into_elements(self) -> Vec<Element>;
}

impl IntoElements for Element {
    fn into_elements(self) -> Vec<Element> {
        vec![self]
    }
}

// `Vec<Element>` / `Vec<T>` / `Option<Element>` / `.map(..)` 결과 등
// "요소를 낼 수 있는 것"은 모두 여기로 통일된다.
impl<I, T> IntoElements for I
where
    I: IntoIterator<Item = T>,
    T: IntoElements,
{
    fn into_elements(self) -> Vec<Element> {
        self.into_iter().flat_map(IntoElements::into_elements).collect()
    }
}

/// `props.on_select(item.id)` — 컴포넌트가 부모에게 값을 올려보내는 콜백
/// (사양서 3.1의 `on_select: fn(Id)`).
///
/// `Rc<dyn Fn>`이라 클론이 싸고, props가 `'static` 클로저를 담을 수 있다.
pub struct Callback<T> {
    f: Rc<dyn Fn(&mut Arena, T)>,
}

impl<T> Clone for Callback<T> {
    fn clone(&self) -> Self {
        Callback { f: Rc::clone(&self.f) }
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
        Callback { f: Rc::new(|_, _| {}) }
    }
}

/// 뷰 본문의 값 → 단일 `Element`.
///
/// 본문이 요소 목록(`Vec<Element>`)이면 `Fragment`로 감싼다 —
/// `#[view] fn App() { {if ..} }` 같이 본문 전체가 조건부인 경우를 위해서다.
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
        Element::Fragment { children: self }
    }
}
