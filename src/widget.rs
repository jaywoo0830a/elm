//! Widget protocol (v0.7) — `Element`는 **위젯 구조체들의 enum**이고,
//! `enum_dispatch`가 [`Widget`] 트레이트의 메서드를 각 variant의 구조체로
//! **정적 디스패치**한다.
//!
//! ## 왜 이렇게 바꿨나
//!
//! 0.6까지 `Element`는 필드를 인라인으로 가진 데이터 enum이었고, 소비 측마다
//! 거대한 `match`가 흩어져 있었다:
//!
//! - `element.rs::collect_texts` · `children` · `tag` · `class`
//! - `testing.rs::dump` · `find_button` · `find_input_change` · `find_check` ...
//! - `raw.rs::invoke`
//! - egui 어댑터의 `render_el` · `is_disabled`
//!
//! 위젯을 하나 추가하려면 이 여섯 곳을 **동시에** 고쳐야 했고, 하나를 빠뜨리면
//! 조용히 동작이 어긋났다.
//!
//! v0.7부터 위젯의 능력은 [`Widget`] 트레이트 **한 곳**에 선언된다. 새 위젯을
//! 추가하면 컴파일러가 빠진 메서드를 잡아준다. 트리 순회·접근성·테스트 셀렉터는
//! 더 이상 variant를 알 필요가 없다 — `element.role()`, `element.on_click()`처럼
//! 프로토콜에 물어본다.
//!
//! 백엔드(egui 어댑터)만 여전히 variant를 매칭한다. 그건 **그리기**라는 본질적으로
//! 백엔드별 작업이라 visitor 패턴이 맞다.

use crate::element::{BoolHandler, Handler, ValueHandler};
use crate::raw::RawFn;

use enum_dispatch::enum_dispatch;

/// 접근성/셀렉터 역할 — 스크린리더와 테스트가 공유하는 어휘.
///
/// `role=Button`처럼 role로 위젯을 찾으면, 라벨 텍스트가 바뀌어도
/// (`"+"` → `"더하기"`) 테스트가 깨지지 않는다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Group,
    Text,
    Strong,
    Banner,
    Button,
    Tab,
    ColumnHeader,
    Cell,
    TextBox,
    Checkbox,
    Progress,
    Spinner,
    Separator,
    Dialog,
    Raw,
    /// 의미 없는 노드 (태그 없는 Fragment 등).
    None,
}

impl Role {
    /// `a11y_tree()` / 진단용 짧은 이름.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Group => "group",
            Role::Text => "text",
            Role::Strong => "strong",
            Role::Banner => "banner",
            Role::Button => "button",
            Role::Tab => "tab",
            Role::ColumnHeader => "columnheader",
            Role::Cell => "cell",
            Role::TextBox => "textbox",
            Role::Checkbox => "checkbox",
            Role::Progress => "progressbar",
            Role::Spinner => "spinner",
            Role::Separator => "separator",
            Role::Dialog => "dialog",
            Role::Raw => "raw",
            Role::None => "none",
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 위젯이 공통으로 답하는 질문들.
///
/// 기본 구현은 "아무것도 아님"이고, 각 위젯이 자기 능력만 덮어쓴다.
/// `enum_dispatch`가 `Element`의 각 variant로 이 호출들을 위임한다.
#[enum_dispatch]
pub trait Widget {
    /// 사람이 읽는 위젯 이름 (`"Button"`) — 덤프/진단용.
    fn kind(&self) -> &'static str;
    /// `css!`의 태그 셀렉터 이름 (`"button"`). Fragment는 `""`.
    fn tag(&self) -> &'static str;
    /// 이 노드의 클래스 목록.
    fn class(&self) -> &[String];
    /// 자식을 갖는 컨테이너면 그 목록 (아니면 `None`).
    fn children(&self) -> Option<&[Element]>;

    /// 이 노드가 내는 텍스트 조각을 `out`에 붙인다.
    fn texts_into(&self, out: &mut Vec<String>);

    /// 서브트리의 텍스트 노드들.
    fn texts(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.texts_into(&mut out);
        out
    }

    /// 서브트리 텍스트를 이어 붙인다 (클릭 가능한 컨테이너 매칭용).
    fn subtree_text(&self) -> String {
        self.texts().join("")
    }

    /// 접근성 역할.
    fn role(&self) -> Role {
        Role::None
    }

    /// 접근성 라벨 (버튼 텍스트, 입력값, 체크 라벨 …).
    fn label(&self) -> Option<&str> {
        None
    }

    /// 사용자 입력을 받는가.
    fn is_interactive(&self) -> bool {
        false
    }

    /// 비활성 상태인가 (`:disabled` 스타일 판정에도 쓰인다).
    fn is_disabled(&self) -> bool {
        false
    }

    /// 클릭 핸들러 (`Button`/`Tab`/`Th`/`on_click`이 있는 컨테이너).
    fn on_click(&self) -> Option<&Handler> {
        None
    }

    /// 값(문자열) 변경 핸들러 (`Input`/`TextArea`).
    fn on_value_change(&self) -> Option<&ValueHandler> {
        None
    }

    /// Enter 핸들러 (`Input`/`TextArea`).
    fn on_value_enter(&self) -> Option<&ValueHandler> {
        None
    }

    /// bool 변경 핸들러 (`Check`).
    fn on_bool_change(&self) -> Option<&BoolHandler> {
        None
    }

    /// 입력의 현재 값.
    fn value(&self) -> Option<&str> {
        None
    }

    /// 체크 상태.
    fn checked(&self) -> Option<bool> {
        None
    }

    /// `<Raw>`의 불투명 위젯 클로저 (플랫폼 어댑터 전용).
    fn raw_fn(&self) -> Option<&RawFn> {
        None
    }

    /// 들여쓰기 트리 덤프 (스냅샷 계약, 사양서 8.3) — `render_tree()`가 쓴다.
    fn dump(&self, depth: usize, out: &mut String);
}

/// `dump`/`a11y_tree` 공통 들여쓰기.
pub(crate) fn pad(depth: usize) -> String {
    "  ".repeat(depth)
}

/// `dump`의 클래스 표기: ` .a.b` (없으면 빈 문자열).
pub(crate) fn fmt_class(class: &[String]) -> String {
    if class.is_empty() {
        String::new()
    } else {
        format!(" .{}", class.join("."))
    }
}

// ─────────────────────────────────────────────────────────────
// 위젯 구조체들 — 각자 자기 능력만 구현한다.
// ─────────────────────────────────────────────────────────────

/// `<Text>"..."</Text>` — 순수 텍스트 노드.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TextEl {
    pub text: String,
    pub class: Vec<String>,
}

impl Widget for TextEl {
    fn kind(&self) -> &'static str {
        "Text"
    }
    fn tag(&self) -> &'static str {
        "text"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(self.text.clone());
    }
    fn role(&self) -> Role {
        Role::Text
    }
    fn label(&self) -> Option<&str> {
        Some(&self.text)
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Text {:?}{}\n",
            pad(depth),
            self.text,
            fmt_class(&self.class)
        ));
    }
}

/// `<Strong>"..."</Strong>` — 강조 텍스트.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct StrongEl {
    pub text: String,
    pub class: Vec<String>,
}

impl Widget for StrongEl {
    fn kind(&self) -> &'static str {
        "Strong"
    }
    fn tag(&self) -> &'static str {
        "strong"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(self.text.clone());
    }
    fn role(&self) -> Role {
        Role::Strong
    }
    fn label(&self) -> Option<&str> {
        Some(&self.text)
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Strong {:?}{}\n",
            pad(depth),
            self.text,
            fmt_class(&self.class)
        ));
    }
}

/// `<Banner kind="error">"..."</Banner>` — 종류별 색의 알림.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct BannerEl {
    pub kind: String,
    pub text: String,
    pub class: Vec<String>,
}

impl Widget for BannerEl {
    fn kind(&self) -> &'static str {
        "Banner"
    }
    fn tag(&self) -> &'static str {
        "banner"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(self.text.clone());
    }
    fn role(&self) -> Role {
        Role::Banner
    }
    fn label(&self) -> Option<&str> {
        Some(&self.text)
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Banner kind={:?} {:?}{}\n",
            pad(depth),
            self.kind,
            self.text,
            fmt_class(&self.class)
        ));
    }
}

/// `<Button on_click={..} disabled={..}>"..."</Button>`.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ButtonEl {
    pub text: String,
    pub class: Vec<String>,
    pub disabled: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_click: Option<Handler>,
}

impl Widget for ButtonEl {
    fn kind(&self) -> &'static str {
        "Button"
    }
    fn tag(&self) -> &'static str {
        "button"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(self.text.clone());
    }
    fn role(&self) -> Role {
        Role::Button
    }
    fn label(&self) -> Option<&str> {
        Some(&self.text)
    }
    fn is_interactive(&self) -> bool {
        true
    }
    fn is_disabled(&self) -> bool {
        self.disabled
    }
    fn on_click(&self) -> Option<&Handler> {
        self.on_click.as_ref()
    }
    fn dump(&self, depth: usize, out: &mut String) {
        let d = if self.disabled { " disabled" } else { "" };
        out.push_str(&format!(
            "{}Button {:?}{}{}\n",
            pad(depth),
            self.text,
            d,
            fmt_class(&self.class)
        ));
    }
}

/// `<Tab active={..} on_click={..}>"..."</Tab>`.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TabEl {
    pub text: String,
    pub active: bool,
    pub class: Vec<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_click: Option<Handler>,
}

impl Widget for TabEl {
    fn kind(&self) -> &'static str {
        "Tab"
    }
    fn tag(&self) -> &'static str {
        "tab"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(self.text.clone());
    }
    fn role(&self) -> Role {
        Role::Tab
    }
    fn label(&self) -> Option<&str> {
        Some(&self.text)
    }
    fn is_interactive(&self) -> bool {
        true
    }
    fn on_click(&self) -> Option<&Handler> {
        self.on_click.as_ref()
    }
    fn dump(&self, depth: usize, out: &mut String) {
        let a = if self.active { " active" } else { "" };
        out.push_str(&format!(
            "{}Tab {:?}{}{}\n",
            pad(depth),
            self.text,
            a,
            fmt_class(&self.class)
        ));
    }
}

/// `<Th on_click={..}>"..."</Th>` — 표 헤더 셀.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ThEl {
    pub text: String,
    pub class: Vec<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_click: Option<Handler>,
}

impl Widget for ThEl {
    fn kind(&self) -> &'static str {
        "Th"
    }
    fn tag(&self) -> &'static str {
        "th"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(self.text.clone());
    }
    fn role(&self) -> Role {
        Role::ColumnHeader
    }
    fn label(&self) -> Option<&str> {
        Some(&self.text)
    }
    fn is_interactive(&self) -> bool {
        true
    }
    fn on_click(&self) -> Option<&Handler> {
        self.on_click.as_ref()
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Th {:?}{}\n",
            pad(depth),
            self.text,
            fmt_class(&self.class)
        ));
    }
}

/// `<Td>"..."</Td>` — 표 본문 셀.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TdEl {
    pub text: String,
    pub class: Vec<String>,
}

impl Widget for TdEl {
    fn kind(&self) -> &'static str {
        "Td"
    }
    fn tag(&self) -> &'static str {
        "td"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(self.text.clone());
    }
    fn role(&self) -> Role {
        Role::Cell
    }
    fn label(&self) -> Option<&str> {
        Some(&self.text)
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Td {:?}{}\n",
            pad(depth),
            self.text,
            fmt_class(&self.class)
        ));
    }
}

/// `<Input value={..} on_change={..} on_enter={..} />`.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct InputEl {
    pub value: String,
    pub class: Vec<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_change: Option<ValueHandler>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_enter: Option<ValueHandler>,
}

impl Widget for InputEl {
    fn kind(&self) -> &'static str {
        "Input"
    }
    fn tag(&self) -> &'static str {
        "input"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(format!("[input: {}]", self.value));
    }
    fn role(&self) -> Role {
        Role::TextBox
    }
    fn label(&self) -> Option<&str> {
        Some(&self.value)
    }
    fn is_interactive(&self) -> bool {
        true
    }
    fn on_value_change(&self) -> Option<&ValueHandler> {
        self.on_change.as_ref()
    }
    fn on_value_enter(&self) -> Option<&ValueHandler> {
        self.on_enter.as_ref()
    }
    fn value(&self) -> Option<&str> {
        Some(&self.value)
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Input value={:?}{}\n",
            pad(depth),
            self.value,
            fmt_class(&self.class)
        ));
    }
}

/// `<TextArea value={..} on_change={..} on_enter={..} />` — Input과 동일 계약.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TextAreaEl {
    pub value: String,
    pub class: Vec<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_change: Option<ValueHandler>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_enter: Option<ValueHandler>,
}

impl Widget for TextAreaEl {
    fn kind(&self) -> &'static str {
        "TextArea"
    }
    fn tag(&self) -> &'static str {
        "textarea"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(format!("[input: {}]", self.value));
    }
    fn role(&self) -> Role {
        Role::TextBox
    }
    fn label(&self) -> Option<&str> {
        Some(&self.value)
    }
    fn is_interactive(&self) -> bool {
        true
    }
    fn on_value_change(&self) -> Option<&ValueHandler> {
        self.on_change.as_ref()
    }
    fn on_value_enter(&self) -> Option<&ValueHandler> {
        self.on_enter.as_ref()
    }
    fn value(&self) -> Option<&str> {
        Some(&self.value)
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}TextArea value={:?}{}\n",
            pad(depth),
            self.value,
            fmt_class(&self.class)
        ));
    }
}

/// `<Check checked={..} on_change={..}>"라벨"</Check>`.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CheckEl {
    pub checked: bool,
    pub label: String,
    pub class: Vec<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_change: Option<BoolHandler>,
}

impl Widget for CheckEl {
    fn kind(&self) -> &'static str {
        "Check"
    }
    fn tag(&self) -> &'static str {
        "check"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(self.label.clone());
    }
    fn role(&self) -> Role {
        Role::Checkbox
    }
    fn label(&self) -> Option<&str> {
        Some(&self.label)
    }
    fn is_interactive(&self) -> bool {
        true
    }
    fn on_bool_change(&self) -> Option<&BoolHandler> {
        self.on_change.as_ref()
    }
    fn checked(&self) -> Option<bool> {
        Some(self.checked)
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Check {:?} checked={}{}\n",
            pad(depth),
            self.label,
            self.checked,
            fmt_class(&self.class)
        ));
    }
}

/// `<Spinner />` — 진행 중 표시.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SpinnerEl {
    pub class: Vec<String>,
}

impl Widget for SpinnerEl {
    fn kind(&self) -> &'static str {
        "Spinner"
    }
    fn tag(&self) -> &'static str {
        "spinner"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push("[spinner]".to_string());
    }
    fn role(&self) -> Role {
        Role::Spinner
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Spinner{}\n",
            pad(depth),
            fmt_class(&self.class)
        ));
    }
}

/// `<Divider />` — 구분선.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DividerEl {
    pub class: Vec<String>,
}

impl Widget for DividerEl {
    fn kind(&self) -> &'static str {
        "Divider"
    }
    fn tag(&self) -> &'static str {
        "divider"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push("[divider]".to_string());
    }
    fn role(&self) -> Role {
        Role::Separator
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Divider{}\n",
            pad(depth),
            fmt_class(&self.class)
        ));
    }
}

/// `<Progress value={0.5} />`.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ProgressEl {
    pub value: f64,
    pub class: Vec<String>,
}

impl Widget for ProgressEl {
    fn kind(&self) -> &'static str {
        "Progress"
    }
    fn tag(&self) -> &'static str {
        "progress"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push(format!("[progress: {}]", self.value));
    }
    fn role(&self) -> Role {
        Role::Progress
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Progress {}{}\n",
            pad(depth),
            self.value,
            fmt_class(&self.class)
        ));
    }
}

/// `<Raw>|ui: &mut PlatformType| { ... }</Raw>` — 유일한 탈출구 (사양서 7.3).
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct RawEl {
    pub class: Vec<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub widget: RawFn,
}

impl Widget for RawEl {
    fn kind(&self) -> &'static str {
        "Raw"
    }
    fn tag(&self) -> &'static str {
        "raw"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        None
    }
    fn texts_into(&self, _out: &mut Vec<String>) {}
    fn role(&self) -> Role {
        Role::Raw
    }
    fn raw_fn(&self) -> Option<&RawFn> {
        Some(&self.widget)
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!("{}[raw]{}\n", pad(depth), fmt_class(&self.class)));
    }
}

/// `<Col>children</Col>` — 세로 컨테이너.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ColEl {
    pub class: Vec<String>,
    pub children: Vec<Element>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_click: Option<Handler>,
}

impl Widget for ColEl {
    fn kind(&self) -> &'static str {
        "Col"
    }
    fn tag(&self) -> &'static str {
        "col"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        Some(&self.children)
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        for c in &self.children {
            c.texts_into(out);
        }
    }
    fn role(&self) -> Role {
        Role::Group
    }
    fn is_interactive(&self) -> bool {
        self.on_click.is_some()
    }
    fn on_click(&self) -> Option<&Handler> {
        self.on_click.as_ref()
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!("{}Col{}\n", pad(depth), fmt_class(&self.class)));
        for c in &self.children {
            c.dump(depth + 1, out);
        }
    }
}

/// `<Row>children</Row>` — 가로 컨테이너.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct RowEl {
    pub class: Vec<String>,
    pub children: Vec<Element>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_click: Option<Handler>,
}

impl Widget for RowEl {
    fn kind(&self) -> &'static str {
        "Row"
    }
    fn tag(&self) -> &'static str {
        "row"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        Some(&self.children)
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        for c in &self.children {
            c.texts_into(out);
        }
    }
    fn role(&self) -> Role {
        Role::Group
    }
    fn is_interactive(&self) -> bool {
        self.on_click.is_some()
    }
    fn on_click(&self) -> Option<&Handler> {
        self.on_click.as_ref()
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!("{}Row{}\n", pad(depth), fmt_class(&self.class)));
        for c in &self.children {
            c.dump(depth + 1, out);
        }
    }
}

/// `<Modal on_close={..}>children</Modal>` — 자식을 갖는 빌트인.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ModalEl {
    pub title: String,
    pub class: Vec<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub on_close: Option<Handler>,
    pub children: Vec<Element>,
}

impl Widget for ModalEl {
    fn kind(&self) -> &'static str {
        "Modal"
    }
    fn tag(&self) -> &'static str {
        "modal"
    }
    fn class(&self) -> &[String] {
        &self.class
    }
    fn children(&self) -> Option<&[Element]> {
        Some(&self.children)
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        out.push("[modal]".to_string());
        for c in &self.children {
            c.texts_into(out);
        }
    }
    fn role(&self) -> Role {
        Role::Dialog
    }
    fn label(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{}Modal {:?}{}\n",
            pad(depth),
            self.title,
            fmt_class(&self.class)
        ));
        for c in &self.children {
            c.dump(depth + 1, out);
        }
    }
}

/// 뷰 본문/분기가 요소 여러 개를 낼 때의 컨테이너 (`{if ..}`, `{match ..}`).
/// 레이아웃을 갖지 않는 순수 묶음이다.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FragmentEl {
    pub children: Vec<Element>,
}

impl Widget for FragmentEl {
    fn kind(&self) -> &'static str {
        "Fragment"
    }
    fn tag(&self) -> &'static str {
        ""
    }
    fn class(&self) -> &[String] {
        &[]
    }
    fn children(&self) -> Option<&[Element]> {
        Some(&self.children)
    }
    fn texts_into(&self, out: &mut Vec<String>) {
        for c in &self.children {
            c.texts_into(out);
        }
    }
    fn role(&self) -> Role {
        Role::None
    }
    fn dump(&self, depth: usize, out: &mut String) {
        out.push_str(&format!("{}Fragment\n", pad(depth)));
        for c in &self.children {
            c.dump(depth + 1, out);
        }
    }
}

// ─────────────────────────────────────────────────────────────
// Element — 위젯 구조체들의 enum
// ─────────────────────────────────────────────────────────────

/// 뷰 트리. 순수 데이터 — 같은 상태면 같은 트리 (사양서 2.1).
///
/// v0.7: 각 variant는 **위젯 구조체**를 감싸고, `enum_dispatch`가 [`Widget`]의
/// 메서드를 그 구조체로 정적 디스패치한다. 그래서 소비 측(테스트, 접근성,
/// 플랫폼 어댑터)은 더 이상 variant를 몰라도 된다.
///
/// ```ignore
/// let el: Element = elm_magic::ui! { <Button disabled={true}>"go"</Button> };
/// assert_eq!(el.role().as_str(), "button");   // variant 매칭 없이 프로토콜 질의
/// assert!(el.is_disabled());
/// ```
#[enum_dispatch(Widget)]
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Element {
    Text(TextEl),
    Strong(StrongEl),
    Col(ColEl),
    Row(RowEl),
    Button(ButtonEl),
    Input(InputEl),
    TextArea(TextAreaEl),
    Check(CheckEl),
    Tab(TabEl),
    Th(ThEl),
    Td(TdEl),
    Banner(BannerEl),
    Spinner(SpinnerEl),
    Divider(DividerEl),
    Progress(ProgressEl),
    Modal(ModalEl),
    /// `<Raw>` — 유일한 탈출구 (사양서 7.3).
    Raw(RawEl),
    /// 레이아웃 없는 순수 묶음.
    Fragment(FragmentEl),
}

/// 이름으로 위젯 구조체를 찾을 때 쓰는 편의 목록 (테스트/문서).
pub const WIDGET_KINDS: &[&str] = &[
    "Text", "Strong", "Col", "Row", "Button", "Input", "TextArea", "Check", "Tab", "Th", "Td",
    "Banner", "Spinner", "Divider", "Progress", "Modal", "Raw", "Fragment",
];

impl Element {
    /// 원소의 위젯 종류 이름 (`"Button"`) — `Widget::kind`의 별칭.
    pub fn widget_kind(&self) -> &'static str {
        // `enum_dispatch`가 만든 `Widget::kind`로 위임.
        Widget::kind(self)
    }
}
