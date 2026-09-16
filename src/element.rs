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
    /// `<TextArea>` — 여러 줄 입력 (Input과 동일한 이벤트 계약)
    TextArea {
        value: String,
        class: Vec<String>,
        on_change: Option<ValueHandler>,
        on_enter: Option<ValueHandler>,
    },
    /// `<Check checked={..} on_change={..}>"라벨"</Check>`
    Check {
        checked: bool,
        label: String,
        class: Vec<String>,
        on_change: Option<BoolHandler>,
    },
    /// `<Tab active={..} on_click={..}>"라벨"</Tab>`
    Tab {
        text: String,
        active: bool,
        class: Vec<String>,
        on_click: Option<Handler>,
    },
    /// `<Th on_click={..}>` — 표 헤더 셀
    Th {
        text: String,
        class: Vec<String>,
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
        on_close: Option<Handler>,
        children: Vec<Element>,
    },
    /// `<Raw>|ui: &mut PlatformType| { ... }</Raw>` — the escape hatch
    /// (사양서 7.3). Opaque to headless testing; the platform adapter
    /// invokes it with its own handle (e.g. `&mut egui::Ui`).
    Raw {
        class: Vec<String>,
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
