//! 스타일 등록 + 해석 (사양서 6.1, 6.2, 6.3) — v0.6.
//!
//! `css!`가 **컴파일타임에 검증한** 선언을 여기에 등록하고, 렌더러가
//! `Element`의 태그/`class`로 해석한다. 해석 결과(`ResolvedStyle`)는
//! egui를 모르는 값이라 **스타일 계약 전체를 헤드리스 테스트로** 검증할 수 있다.
//!
//! 셀렉터는 CSS와 같은 뜻이다:
//! - `.card` → **클래스** 셀렉터. `Element::class`의 이름과 맞는다.
//! - `button` → **태그** 셀렉터. `Element::tag()`와 맞는다 (대소문자 무시).
//!
//! 둘은 키 공간이 분리되어 있어 `.button { … }`(클래스)와
//! `button { … }`(태그)가 서로를 덮어쓰지 않는다.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// ── 팔레트 (egui 독립) ────────────────────────────────────────

/// 팔레트 토큰 — `bg: surface`, `color: text_dim` 같은 값 (사양서 6.1).
///
/// `Token::ALL`의 순서는 아래 판별값(discriminant)과 **반드시** 같아야 한다
/// (`name()`이 `ALL`을 인덱싱한다).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Primary = 0,
    OnPrimary = 1,
    Surface = 2,
    Background = 3,
    Text = 4,
    TextDim = 5,
    Error = 6,
    Warn = 7,
}

impl Token {
    /// (이름, 토큰). `crates/elm-magic-macros/src/css.rs`의 토큰 목록과 같아야 한다.
    pub const ALL: &'static [(&'static str, Token)] = &[
        ("primary", Token::Primary),
        ("on_primary", Token::OnPrimary),
        ("surface", Token::Surface),
        ("background", Token::Background),
        ("text", Token::Text),
        ("text_dim", Token::TextDim),
        ("error", Token::Error),
        ("warn", Token::Warn),
    ];

    /// 이름 → 토큰 (`css!`가 쓰는 어휘).
    pub fn from_name(name: &str) -> Option<Token> {
        Token::ALL.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
    }

    /// 토큰 → 이름 (선언 텍스트 복원용).
    pub fn name(self) -> &'static str {
        Token::ALL[self as usize].0
    }
}

/// 색 (egui 독립) — 어댑터가 플랫폼 색으로 바꾼다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// 팔레트 (사양서 6.3 테마의 기반).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    colors: [Color; 8],
}

impl Palette {
    /// 토큰의 색.
    pub fn get(&self, token: Token) -> Color {
        self.colors[token as usize]
    }

    /// 토큰의 색을 바꾼다 (테마 만들기).
    pub fn set(&mut self, token: Token, color: Color) {
        self.colors[token as usize] = color;
    }

    /// 사슬형 테마 만들기: `Palette::dark().with(Token::Primary, Color::rgb(…))`.
    pub fn with(mut self, token: Token, color: Color) -> Self {
        self.set(token, color);
        self
    }

    /// 어두운 테마 (egui 기본과 맞춘 값).
    pub fn dark() -> Self {
        Self {
            colors: [
                Color::rgb(59, 130, 246),  // primary
                Color::rgb(255, 255, 255), // on_primary
                Color::rgb(39, 39, 42),    // surface
                Color::rgb(24, 24, 27),    // background
                Color::rgb(228, 228, 231), // text
                Color::rgb(161, 161, 170), // text_dim
                Color::rgb(239, 68, 68),   // error
                Color::rgb(245, 158, 11),  // warn
            ],
        }
    }

    /// 밝은 테마.
    pub fn light() -> Self {
        Self {
            colors: [
                Color::rgb(37, 99, 235),   // primary
                Color::rgb(255, 255, 255), // on_primary
                Color::rgb(255, 255, 255), // surface
                Color::rgb(250, 250, 250), // background
                Color::rgb(24, 24, 27),    // text
                Color::rgb(113, 113, 122), // text_dim
                Color::rgb(220, 38, 38),   // error
                Color::rgb(217, 119, 6),   // warn
            ],
        }
    }
}

/// 상하좌우 값 — CSS 숏핸드 `padding: 8` / `padding: 8 16` / `padding: 8 16 8 16`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    /// 네 방향 같은 값.
    pub const fn splat(value: f32) -> Self {
        Self { top: value, right: value, bottom: value, left: value }
    }

    /// `상하 좌우` (CSS 2값 숏핸드).
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self { top: vertical, right: horizontal, bottom: vertical, left: horizontal }
    }

    /// `top right bottom left` (CSS 4값 숏핸드).
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self { top, right, bottom, left }
    }

    /// 선언 텍스트 복원: `"16"` / `"8 16"` / `"8 16 8 16"`.
    pub fn text(&self) -> String {
        if self.top == self.right && self.right == self.bottom && self.bottom == self.left {
            format!("{}", self.top)
        } else if self.top == self.bottom && self.left == self.right {
            format!("{} {}", self.top, self.right)
        } else {
            format!("{} {} {} {}", self.top, self.right, self.bottom, self.left)
        }
    }
}

// ── 선언(StyleSpec) / 확정(ResolvedStyle) ────────────────────

/// `css!`가 선언한 스타일 — 팔레트 해석 **전** 상태 (토큰).
///
/// `css!`는 이 구조체를 직접 만든다: 선언하지 않은 필드는 `None`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleSpec {
    pub gap: Option<f32>,
    pub padding: Option<Edges>,
    pub margin: Option<Edges>,
    pub bg: Option<Token>,
    pub color: Option<Token>,
    pub radius: Option<f32>,
    pub font_size: Option<f32>,
    /// `weight: bold` / `weight: normal`
    pub bold: Option<bool>,
    /// `text-decoration: line-through`
    pub strike: Option<bool>,
    /// `text-decoration: underline`
    pub underline: Option<bool>,
}

impl StyleSpec {
    /// 아무것도 선언하지 않은 값 — `css!`가 `..StyleSpec::NONE`으로 쓴다.
    pub const NONE: StyleSpec = StyleSpec {
        gap: None,
        padding: None,
        margin: None,
        bg: None,
        color: None,
        radius: None,
        font_size: None,
        bold: None,
        strike: None,
        underline: None,
    };
}

impl Default for StyleSpec {
    fn default() -> Self {
        Self::NONE
    }
}

/// 해석이 끝난 확정 스타일 (egui 독립) — 어댑터가 그대로 쓴다.
///
/// 색은 이미 팔레트로 확정되어 있어 어댑터가 팔레트를 몰라도 된다.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResolvedStyle {
    pub gap: Option<f32>,
    pub padding: Option<Edges>,
    pub margin: Option<Edges>,
    pub bg: Option<Color>,
    pub color: Option<Color>,
    pub radius: Option<f32>,
    pub font_size: Option<f32>,
    pub bold: Option<bool>,
    pub strike: Option<bool>,
    pub underline: Option<bool>,
}

impl ResolvedStyle {
    /// 선언이 하나도 없으면 true.
    pub fn is_empty(&self) -> bool {
        *self == ResolvedStyle::default()
    }

    /// 한 셀렉터의 선언을 덮어쓴다 — **나중에 적용된 것이 이긴다** (캐스케이드).
    pub fn apply(&mut self, spec: &StyleSpec, palette: &Palette) {
        if let Some(v) = spec.gap {
            self.gap = Some(v);
        }
        if let Some(v) = spec.padding {
            self.padding = Some(v);
        }
        if let Some(v) = spec.margin {
            self.margin = Some(v);
        }
        if let Some(v) = spec.bg {
            self.bg = Some(palette.get(v));
        }
        if let Some(v) = spec.color {
            self.color = Some(palette.get(v));
        }
        if let Some(v) = spec.radius {
            self.radius = Some(v);
        }
        if let Some(v) = spec.font_size {
            self.font_size = Some(v);
        }
        if let Some(v) = spec.bold {
            self.bold = Some(v);
        }
        if let Some(v) = spec.strike {
            self.strike = Some(v);
        }
        if let Some(v) = spec.underline {
            self.underline = Some(v);
        }
    }
}

// ── 등록부 ───────────────────────────────────────────────────

/// 등록된 셀렉터 하나.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleProps {
    selector: String,
    spec: StyleSpec,
}

impl StyleProps {
    /// 사용자가 쓴 그대로의 셀렉터 (`.card` / `button`).
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// 선언된 스타일.
    pub fn spec(&self) -> &StyleSpec {
        &self.spec
    }

    /// 선언된 속성들을 **선언 텍스트**로 돌려준다 (디버깅·직렬화용).
    pub fn pairs(&self) -> Vec<(String, String)> {
        let s = &self.spec;
        let mut out: Vec<(String, String)> = Vec::new();
        if let Some(v) = s.gap {
            out.push(("gap".into(), format!("{}", v)));
        }
        if let Some(v) = s.padding {
            out.push(("padding".into(), v.text()));
        }
        if let Some(v) = s.margin {
            out.push(("margin".into(), v.text()));
        }
        if let Some(v) = s.bg {
            out.push(("bg".into(), v.name().into()));
        }
        if let Some(v) = s.color {
            out.push(("color".into(), v.name().into()));
        }
        if let Some(v) = s.radius {
            out.push(("radius".into(), format!("{}", v)));
        }
        if let Some(v) = s.font_size {
            out.push(("font-size".into(), format!("{}", v)));
        }
        if let Some(v) = s.bold {
            out.push(("weight".into(), if v { "bold" } else { "normal" }.into()));
        }
        if let Some(v) = s.strike {
            out.push((
                "text-decoration".into(),
                if v { "line-through" } else { "none" }.into(),
            ));
        } else if let Some(v) = s.underline {
            out.push((
                "text-decoration".into(),
                if v { "underline" } else { "none" }.into(),
            ));
        }
        out
    }

    /// 선언 텍스트 조회 — `get("gap") == Some("8")`.
    pub fn get(&self, key: &str) -> Option<String> {
        self.pairs()
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for StyleProps {
    /// 선언 **텍스트**로 직렬화한다 (스냅샷이 사람이 읽을 수 있게).
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let props: std::collections::BTreeMap<String, String> = self.pairs().into_iter().collect();
        let mut st = serializer.serialize_struct("StyleProps", 2)?;
        st.serialize_field("selector", &self.selector)?;
        st.serialize_field("props", &props)?;
        st.end()
    }
}

// ── 등록 / 조회 / 해석 ────────────────────────────────────────

/// 등록 키 — 클래스와 태그의 키 공간을 분리한다.
fn key_of(selector: &str) -> String {
    match selector.strip_prefix('.') {
        Some(class) => format!("class:{class}"),
        None => format!("tag:{}", selector.to_lowercase()),
    }
}

fn registry() -> &'static Mutex<HashMap<String, StyleProps>> {
    static REG: OnceLock<Mutex<HashMap<String, StyleProps>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register selectors (called by `css!`). 같은 셀렉터는 **첫 등록이 이긴다**.
pub fn register(entries: Vec<(&str, StyleSpec)>) {
    let mut reg = registry().lock().unwrap();
    for (selector, spec) in entries {
        reg.entry(key_of(selector))
            .or_insert(StyleProps { selector: selector.to_string(), spec });
    }
}

/// CSS 뜻 그대로의 조회: `.x`는 클래스, 그 외는 태그.
pub fn lookup(selector: &str) -> Option<StyleProps> {
    registry().lock().unwrap().get(&key_of(selector)).cloned()
}

/// 클래스 셀렉터 조회 (`"card"`, `".card"` 모두 허용) — `Element::class`의 이름.
pub fn lookup_class(name: &str) -> Option<StyleProps> {
    lookup(&format!(".{}", name.strip_prefix('.').unwrap_or(name)))
}

/// 태그 셀렉터 조회 (`"button"`, 대소문자 무시) — `Element::tag()`의 이름.
pub fn lookup_tag(name: &str) -> Option<StyleProps> {
    let bare = name.strip_prefix('.').unwrap_or(name);
    registry()
        .lock()
        .unwrap()
        .get(&format!("tag:{}", bare.to_lowercase()))
        .cloned()
}

/// 등록된 셀렉터 수 (테스트·디버깅용).
pub fn len() -> usize {
    registry().lock().unwrap().len()
}

/// 태그 → 클래스 순으로 합쳐 최종 스타일을 만든다 (사양서 6.1·6.2).
///
/// 우선순위: 태그 셀렉터 < 클래스 (나열 순서, **뒤가 이긴다**).
pub fn resolve(classes: &[String], tag: &str, palette: &Palette) -> ResolvedStyle {
    let mut out = ResolvedStyle::default();
    if !tag.is_empty() {
        if let Some(props) = lookup_tag(tag) {
            out.apply(props.spec(), palette);
        }
    }
    for class in classes {
        if let Some(props) = lookup_class(class) {
            out.apply(props.spec(), palette);
        }
    }
    out
}

// ── `class={…}` (사양서 6.2 — 조건부 스타일은 값이다) ────────

/// `class={…}`의 값 → 클래스 목록.
///
/// 리터럴(`class="a b"`)은 매크로가 컴파일타임에 나누고,
/// 표현식(`class={if error { "error" } else { "ok" }}`)은 이 트레이트가 받는다.
pub trait IntoClasses {
    fn into_classes(self) -> Vec<String>;
}

impl IntoClasses for &str {
    fn into_classes(self) -> Vec<String> {
        self.split_whitespace().map(str::to_string).collect()
    }
}

impl IntoClasses for String {
    fn into_classes(self) -> Vec<String> {
        self.split_whitespace().map(str::to_string).collect()
    }
}

impl IntoClasses for &String {
    fn into_classes(self) -> Vec<String> {
        self.as_str().into_classes()
    }
}

impl IntoClasses for Vec<String> {
    fn into_classes(self) -> Vec<String> {
        self
    }
}

impl IntoClasses for Vec<&str> {
    fn into_classes(self) -> Vec<String> {
        self.into_iter().flat_map(IntoClasses::into_classes).collect()
    }
}

impl<const N: usize> IntoClasses for [&str; N] {
    fn into_classes(self) -> Vec<String> {
        self.into_iter().flat_map(IntoClasses::into_classes).collect()
    }
}
