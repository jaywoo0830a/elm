//! 스타일 — 렉터 매칭 · 캐스케이드 · 상속 · 팔레트 (사양서 6.1~6.3) — v0.6.
//!
//! `css!`가 **컴파일타임에 검증한** 선언을 여기에 등록하면, 엘리먼트 트리에서
//! **CSS처럼** 해석한다:
//!
//! | 문법 | 뜻 |
//! |---|---|
//! | `button` | **태그** (`Element::tag()`, 대소문자 무시) |
//! | `.card` | **클래스** (`Element::class`) |
//! | `*` | 전부 |
//! | `.card.muted` | 클래스를 **모두** 가진 리먼트 |
//! | `.card Button` | **후손** (아래 어디든) |
//! | `Col > Row` | **직계 자식** |
//! | `.a, .b` | 셀렉터 목록 (같은 선언을 둘 다에) |
//! | `button:hover` | **상태**: `:hover` `:active` `:focus` `:disabled` |
//!
//! 캐스이드는 CSS를 따른다: **명시도(specificity) → 선언 순서**.
//! 명시도는 `(클래스+상태 수, 태그 수)`, `*`는 0이다.
//!
//! `color` `font-size` `weight` `font-style` `font-family` `line-height`
//! `letter-spacing` `text-decoration` `text-align` `text-transform`은 **상속**된다.
//!
//! 해석 결과(`ResolvedStyle`)는 egui를 모르는 값이라 **계약 전체를 헤드리스로**
//! 검증할 수 있다 — 어댑터는 값을 옮기기만 한다.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// ── 팔레트 (플랫폼 독립) ──────────────────────────────────────

/// 팔레트 토큰 — `bg: surface`, `color: text_dim` 같은 값 (사양서 6.1).
///
/// `Token::ALL`의 순서는 판별값과 **반드시** 같아야 한다 (`name()`이 인덱싱한다).
/// `crates/elm-magic-macros/src/css.rs`의 토큰 목록과도 같아야 한다 —
/// `tests/css.rs::css_token_vocabulary_matches_core`가 지킨다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Primary = 0,
    OnPrimary = 1,
    Surface = 2,
    SurfaceAlt = 3,
    Background = 4,
    Text = 5,
    TextDim = 6,
    Error = 7,
    Warn = 8,
    Success = 9,
    Info = 10,
    Border = 11,
    Shadow = 12,
    Overlay = 13,
}

impl Token {
    /// (이름, 토큰) — 순서가 판별값과 같다.
    pub const ALL: &'static [(&'static str, Token)] = &[
        ("primary", Token::Primary),
        ("on_primary", Token::OnPrimary),
        ("surface", Token::Surface),
        ("surface_alt", Token::SurfaceAlt),
        ("background", Token::Background),
        ("text", Token::Text),
        ("text_dim", Token::TextDim),
        ("error", Token::Error),
        ("warn", Token::Warn),
        ("success", Token::Success),
        ("info", Token::Info),
        ("border", Token::Border),
        ("shadow", Token::Shadow),
        ("overlay", Token::Overlay),
    ];

    /// 토큰 수.
    pub const COUNT: usize = Token::ALL.len();

    /// 이름 → 토큰 (`css!`가 쓰는 어휘).
    pub fn from_name(name: &str) -> Option<Token> {
        Token::ALL.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
    }

    /// 토큰 → 이름 (선언 텍스트 복원용).
    pub fn name(self) -> &'static str {
        Token::ALL[self as usize].0
    }
}

/// 색 (플랫폼 독립) — 어댑터가 플랫폼 색으로 바꾼다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// 불투명 색.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// 투명도 포함 색 (`shadow` / `overlay` 토용).
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// 같은 색, 다른 알파.
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }
}

/// 팔레트 (사양서 6.3 — 테마는 팔레트다).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    colors: [Color; Token::COUNT],
}

impl Palette {
    /// 토큰의 색.
    pub fn get(&self, token: Token) -> Color {
        self.colors[token as usize]
    }

    /// 토큰의 색을 바다.
    pub fn set(&mut self, token: Token, color: Color) {
        self.colors[token as usize] = color;
    }

    /// 사슬형 테마: `Palette::dark().with(Token::Primary, Color::rgb(1, 2, 3))`.
    pub fn with(mut self, token: Token, color: Color) -> Self {
        self.set(token, color);
        self
    }

    /// 어두운 테마 (egui 기본과 맞춘 값).
    pub fn dark() -> Self {
        Self {
            colors: [
                Color::rgb(59, 130, 246),     // primary
                Color::rgb(255, 255, 255),    // on_primary
                Color::rgb(39, 39, 42),       // surface
                Color::rgb(63, 63, 70),       // surface_alt
                Color::rgb(24, 24, 27),       // background
                Color::rgb(228, 228, 231),    // text
                Color::rgb(161, 161, 170),    // text_dim
                Color::rgb(239, 68, 68),      // error
                Color::rgb(245, 158, 11),     // warn
                Color::rgb(34, 197, 94),      // success
                Color::rgb(56, 189, 248),     // info
                Color::rgba(82, 82, 91, 180), // border
                Color::rgba(0, 0, 0, 160),    // shadow
                Color::rgba(0, 0, 0, 120),    // overlay
            ],
        }
    }

    /// 밝은 테마.
    pub fn light() -> Self {
        Self {
            colors: [
                Color::rgb(37, 99, 235),         // primary
                Color::rgb(255, 255, 255),       // on_primary
                Color::rgb(255, 255, 255),       // surface
                Color::rgb(244, 244, 245),       // surface_alt
                Color::rgb(250, 250, 250),       // background
                Color::rgb(24, 24, 27),          // text
                Color::rgb(113, 113, 122),       // text_dim
                Color::rgb(220, 38, 38),         // error
                Color::rgb(217, 119, 6),         // warn
                Color::rgb(22, 163, 74),         // success
                Color::rgb(2, 132, 199),         // info
                Color::rgba(212, 212, 216, 255), // border
                Color::rgba(0, 0, 0, 40),        // shadow
                Color::rgba(0, 0, 0, 60),        // overlay
            ],
        }
    }
}

// ── 값 타입 (플랫폼 독립) ────────────────────────────

/// 상하좌우 값 — CSS 숏핸드 `padding: 8` / `8 16` / `8 16 8 16`.
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
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// `상하 좌우` (CSS 2값 숏핸드).
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// `top right bottom left` (CSS 4값 숏핸드).
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// 가로 합.
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// 세로 합.
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
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

/// 길이 — `width: 200` / `fill` / `auto`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Len {
    /// 고정 값 (`200`). `100%`는 `fill`을 쓴다.
    Px(f32),
    /// 부모가 주는 만큼 (`fill`, `100%`).
    Fill,
    /// 내용에 맞춤 (`auto`).
    Auto,
}

impl Len {
    /// 선언 텍스트 복원.
    pub fn text(&self) -> String {
        match self {
            Len::Px(v) => format!("{v}"),
            Len::Fill => "fill".to_string(),
            Len::Auto => "auto".to_string(),
        }
    }
}

/// 정렬 (플랫폼 독립) — `align`(items) / `justify`(content) / `text-align`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

impl Align {
    /// `css!` 값 → 정렬.
    pub fn from_name(name: &str) -> Option<Align> {
        match name {
            "start" | "left" | "top" | "flex-start" => Some(Align::Start),
            "center" => Some(Align::Center),
            "end" | "right" | "bottom" | "flex-end" => Some(Align::End),
            _ => None,
        }
    }

    /// 선언 텍스트 복원.
    pub fn text(self) -> &'static str {
        match self {
            Align::Start => "start",
            Align::Center => "center",
            Align::End => "end",
        }
    }
}

/// `text-transform` 값.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transform {
    None,
    Upper,
    Lower,
    Capitalize,
}

impl Transform {
    /// `css!` 값 → 변환.
    pub fn from_name(name: &str) -> Option<Transform> {
        match name {
            "none" => Some(Transform::None),
            "uppercase" | "upper" => Some(Transform::Upper),
            "lowercase" | "lower" => Some(Transform::Lower),
            "capitalize" => Some(Transform::Capitalize),
            _ => None,
        }
    }

    /// 글자에 적용한다 (어댑터가 렌더 직전에 호출).
    pub fn apply(self, text: &str) -> String {
        match self {
            Transform::None => text.to_string(),
            Transform::Upper => text.to_uppercase(),
            Transform::Lower => text.to_lowercase(),
            Transform::Capitalize => {
                let mut out = String::with_capacity(text.len());
                let mut start = true;
                for c in text.chars() {
                    if c.is_whitespace() {
                        start = true;
                        out.push(c);
                    } else if start {
                        out.extend(c.to_uppercase());
                        start = false;
                    } else {
                        out.push(c);
                    }
                }
                out
            }
        }
    }

    /// 선언 텍스트 복원.
    pub fn text(self) -> &'static str {
        match self {
            Transform::None => "none",
            Transform::Upper => "uppercase",
            Transform::Lower => "lowercase",
            Transform::Capitalize => "capitalize",
        }
    }
}

/// `cursor` 값 — 어댑터가 플랫폼 커서로 옮다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Default,
    Pointer,
    Text,
    Grab,
    Grabbing,
    Move,
    Crosshair,
    NotAllowed,
    None,
}

impl Cursor {
    /// `css!` 값 → 커서.
    pub fn from_name(name: &str) -> Option<Cursor> {
        match name {
            "default" | "auto" => Some(Cursor::Default),
            "pointer" | "hand" | "click" => Some(Cursor::Pointer),
            "text" | "ibeam" => Some(Cursor::Text),
            "grab" => Some(Cursor::Grab),
            "grabbing" => Some(Cursor::Grabbing),
            "move" | "all-scroll" => Some(Cursor::Move),
            "crosshair" => Some(Cursor::Crosshair),
            "not-allowed" | "no-drop" => Some(Cursor::NotAllowed),
            "none" => Some(Cursor::None),
            _ => None,
        }
    }

    /// 선언 텍스트 복원.
    pub fn text(self) -> &'static str {
        match self {
            Cursor::Default => "default",
            Cursor::Pointer => "pointer",
            Cursor::Text => "text",
            Cursor::Grab => "grab",
            Cursor::Grabbing => "grabbing",
            Cursor::Move => "move",
            Cursor::Crosshair => "crosshair",
            Cursor::NotAllowed => "not-allowed",
            Cursor::None => "none",
        }
    }
}

/// 그림자 — CSS `box-shadow`의 `x y blur spread` (뒤 값은 생략 가능).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub dx: f32,
    pub dy: f32,
    pub blur: f32,
    pub spread: f32,
}

impl Shadow {
    /// `y` / `x y` / `x y blur` / `x y blur spread`.
    pub fn from_values(values: &[f32]) -> Option<Shadow> {
        let g = |i: usize| values.get(i).copied().unwrap_or(0.0);
        match values.len() {
            1 => Some(Shadow {
                dx: 0.0,
                dy: g(0),
                blur: g(1),
                spread: 0.0,
            }),
            2 => Some(Shadow {
                dx: g(0),
                dy: g(1),
                blur: 0.0,
                spread: 0.0,
            }),
            3 => Some(Shadow {
                dx: g(0),
                dy: g(1),
                blur: g(2),
                spread: 0.0,
            }),
            4 => Some(Shadow {
                dx: g(0),
                dy: g(1),
                blur: g(2),
                spread: g(3),
            }),
            _ => None,
        }
    }

    /// 선언 텍스트 복원.
    pub fn text(&self) -> String {
        if self.spread != 0.0 {
            format!("{} {} {} {}", self.dx, self.dy, self.blur, self.spread)
        } else if self.blur != 0.0 {
            format!("{} {} {}", self.dx, self.dy, self.blur)
        } else {
            format!("{} {}", self.dx, self.dy)
        }
    }
}

// ── 셀렉터 (CSS 문법) ────────────────────────────────────────

/// 한 노드의 **상호작용 상태** — 어댑터가 채운다 (`ResolvedState`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
    pub disabled: bool,
}

impl State {
    /// 전부 꺼진 상태 (헤드리스 기본).
    pub const NONE: State = State {
        hovered: false,
        active: false,
        focused: false,
        disabled: false,
    };

    /// 어댑터용 생성기.
    pub const fn new(hovered: bool, active: bool, focused: bool, disabled: bool) -> State {
        State {
            hovered,
            active,
            focused,
            disabled,
        }
    }
}

/// 렉터가 **요구하는** 상태 (`:hover` 등). 여러 개면 모두 만족해야 한다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateMask {
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
    pub disabled: bool,
}

impl StateMask {
    /// 요구하는 상태가 없으면 true.
    pub fn is_empty(self) -> bool {
        self == StateMask::default()
    }

    /// 명시도 가산 (CSS에서 의사 클래스는 클래스와 같은 무게).
    pub fn count(self) -> u32 {
        self.hovered as u32 + self.active as u32 + self.focused as u32 + self.disabled as u32
    }

    /// 노드 상태가 이 요구를 만족하는가.
    pub fn matches(self, state: State) -> bool {
        (!self.hovered || state.hovered)
            && (!self.active || state.active)
            && (!self.focused || state.focused)
            && (!self.disabled || state.disabled)
    }

    /// 선언 스트 복원: `":hover:active"`.
    pub fn text(self) -> String {
        let mut out = String::new();
        if self.hovered {
            out.push_str(":hover");
        }
        if self.active {
            out.push_str(":active");
        }
        if self.focused {
            out.push_str(":focus");
        }
        if self.disabled {
            out.push_str(":disabled");
        }
        out
    }
}

impl std::ops::BitOrAssign for StateMask {
    /// 셀렉터 목록(`.a:hover, .b:focus`)의 요구를 합칠 때 쓴다.
    fn bitor_assign(&mut self, rhs: StateMask) {
        self.hovered |= rhs.hovered;
        self.active |= rhs.active;
        self.focused |= rhs.focused;
        self.disabled |= rhs.disabled;
    }
}

impl std::ops::BitOr for StateMask {
    type Output = StateMask;

    fn bitor(mut self, rhs: StateMask) -> StateMask {
        self |= rhs;
        self
    }
}

/// 결합자 — 셀렉터 마디 사이의 관계.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comb {
    /// 후손 (공백): `.card Button`
    Descendant,
    /// 직계 자식: `.card > Button`
    Child,
}

/// 셀렉터 한 마디: 태그(소문자) + 클래스들. `*`는 둘 다 비어 있다.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Part {
    pub tag: Option<String>,
    pub classes: Vec<String>,
}

impl Part {
    /// 이 마디가 노드와 맞는가 (태그 + 클래스 **모두**).
    pub fn matches(&self, node: &Node) -> bool {
        if let Some(tag) = &self.tag {
            if !tag.eq_ignore_ascii_case(node.tag) {
                return false;
            }
        }
        self.classes.iter().all(|c| node.has_class(c))
    }

    /// 선언 텍스트 복원.
    pub fn text(&self) -> String {
        let mut out = String::new();
        if let Some(tag) = &self.tag {
            out.push_str(tag);
        } else if self.classes.is_empty() {
            out.push('*');
        }
        for c in &self.classes {
            out.push('.');
            out.push_str(c);
        }
        out
    }
}

/// 매칭에 필요한 노드 정보 (`Element`에서 만든다).
#[derive(Clone, Copy, Debug)]
pub struct Node<'a> {
    pub tag: &'a str,
    pub classes: &'a [String],
}

impl<'a> Node<'a> {
    /// 어댑터/코어가 직접 만든다.
    pub fn new(tag: &'a str, classes: &'a [String]) -> Self {
        Node { tag, classes }
    }

    /// 클래스를 가졌는가 (점은 여도 안 붙여도 같다).
    pub fn has_class(&self, name: &str) -> bool {
        let bare = name.strip_prefix('.').unwrap_or(name);
        self.classes
            .iter()
            .any(|c| c.strip_prefix('.').unwrap_or(c) == bare)
    }
}

/// 파싱된 터. `path[0]`(자기 자신)부터 오른쪽으로 매칭한다.
#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    /// 마디 (조상 → 자신).
    pub parts: Vec<Part>,
    /// `combs[i]`는 `parts[i+1]` ****의 결합자 (길이 `parts.len()-1`).
    pub combs: Vec<Comb>,
    /// 자신에게 요구하는 상태 (`:hover` 등).
    pub state: StateMask,
    /// 원본 텍스트.
    pub text: String,
}

impl Selector {
    /// 셀렉터 하나를 파싱한다. 잘못되면 `None`.
    pub fn parse(text: &str) -> Option<Selector> {
        let mut seq: Vec<(Comb, Part)> = Vec::new();
        let mut state = StateMask::default();
        let mut rest = text.trim();
        if rest.is_empty() {
            return None;
        }
        let mut comb = Comb::Descendant;
        while !rest.is_empty() {
            if let Some(r) = rest.strip_prefix('>') {
                if seq.is_empty() {
                    return None; // `>`로 시작할 수 없다
                }
                comb = Comb::Child;
                rest = r.trim_start();
                if rest.is_empty() {
                    return None;
                }
            } else if !seq.is_empty() {
                comb = Comb::Descendant;
            }
            let end = rest
                .find(|c: char| c == '>' || c.is_whitespace())
                .unwrap_or(rest.len());
            let (part, mask) = parse_compound(&rest[..end])?;
            state |= mask;
            seq.push((comb, part));
            rest = rest[end..].trim_start();
        }
        if seq.is_empty() {
            return None;
        }
        Some(Selector {
            parts: seq.iter().map(|(_, p)| p.clone()).collect(),
            combs: seq.iter().skip(1).map(|(c, _)| *c).collect(),
            state,
            text: text.trim().to_string(),
        })
    }

    /// CSS 명시도 — `(클래스 + 상태, 태그)`. `*`는 아무것도 세지 않는다.
    pub fn specificity(&self) -> (u32, u32) {
        let mut classes = self.state.count();
        let mut tags = 0;
        for part in &self.parts {
            classes += part.classes.len() as u32;
            if part.tag.is_some() {
                tags += 1;
            }
        }
        (classes, tags)
    }

    /// `path[0]` = 자기 자신, 이후 **조상**(가까운 순).
    ///
    /// 오른쪽(자신)부터 왼쪽(조상)으로 맞춰 본다 — CSS와 같은 방식이라
    /// 조상 없이는 후손 셀렉터가 절대 안 맞는다.
    pub fn matches(&self, path: &[Node], state: State) -> bool {
        let Some(first) = path.first() else {
            return false;
        };
        if !self.state.matches(state) {
            return false;
        }
        let Some(last) = self.parts.last() else {
            return false;
        };
        if !last.matches(first) {
            return false;
        }
        let mut ai = 1;
        let mut pi = self.parts.len() - 1;
        while pi > 0 {
            pi -= 1;
            let part = &self.parts[pi];
            match self.combs[pi] {
                Comb::Child => {
                    if ai >= path.len() || !part.matches(&path[ai]) {
                        return false;
                    }
                    ai += 1;
                }
                Comb::Descendant => {
                    let mut found = false;
                    while ai < path.len() {
                        let hit = part.matches(&path[ai]);
                        ai += 1;
                        if hit {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// 마디가 하나이고 클래스/상태가 없으면 그 태그 이름.
    pub fn sole_tag(&self) -> Option<&str> {
        if self.parts.len() == 1 && self.state.is_empty() && self.parts[0].classes.is_empty() {
            self.parts[0].tag.as_deref()
        } else {
            None
        }
    }

    /// 선언 텍스트 복원.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                out.push_str(match self.combs[i - 1] {
                    Comb::Child => " > ",
                    Comb::Descendant => " ",
                });
            }
            out.push_str(&part.text());
        }
        out.push_str(&self.state.text());
        out
    }
}

/// 마디 하나(`.card`, `Button:hover`, `*`)를 파싱한다.
fn parse_compound(text: &str) -> Option<(Part, StateMask)> {
    let cs: Vec<char> = text.chars().collect();
    let mut part = Part::default();
    let mut state = StateMask::default();
    let mut seen = false;
    let mut tag_seen = false;
    let mut i = 0;
    while i < cs.len() {
        match cs[i] {
            '.' => {
                i += 1;
                let name = ident(&cs, &mut i);
                if name.is_empty() {
                    return None;
                }
                part.classes.push(name);
                seen = true;
            }
            ':' => {
                i += 1;
                let name = ident(&cs, &mut i);
                match name.as_str() {
                    "hover" => state.hovered = true,
                    "active" => state.active = true,
                    "focus" | "focused" => state.focused = true,
                    "disabled" => state.disabled = true,
                    _ => return None,
                }
                seen = true;
            }
            '*' => {
                if seen {
                    return None; // `*`는 마디에 혼자만 온다
                }
                i += 1;
                tag_seen = true;
                seen = true;
            }
            c if c.is_alphanumeric() || c == '_' => {
                if tag_seen {
                    return None; // 태그는 마디에 하나만
                }
                let name = ident(&cs, &mut i);
                part.tag = Some(name.to_lowercase());
                tag_seen = true;
                seen = true;
            }
            _ => return None,
        }
    }
    if !seen {
        return None;
    }
    Some((part, state))
}

/// 식별자 하나를 읽는다 (`-` 포함, 숫자로 시작해도 된다).
fn ident(cs: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < cs.len() {
        let c = cs[*i];
        if c.is_alphanumeric() || c == '_' || c == '-' {
            *i += 1;
        } else {
            break;
        }
    }
    cs[start..*i].iter().collect()
}

// ── 선언(StyleSpec) / 확정(ResolvedStyle) ────────────────────

/// `css!`가 선언한 스타일 — 팔레트 해석 **전** (토큰 그대로).
///
/// 선언하지 않은 필드는 `None`. `css!`는 `..StyleSpec::NONE`으로 쓴다.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleSpec {
    // 레이아웃
    pub gap: Option<f32>,
    pub row_gap: Option<f32>,
    pub column_gap: Option<f32>,
    pub padding: Option<Edges>,
    pub margin: Option<Edges>,
    pub width: Option<Len>,
    pub height: Option<Len>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub align: Option<Align>,
    pub justify: Option<Align>,
    pub wrap: Option<bool>,
    /// `display: none` — 자리를 차지하지 않는다.
    pub display_none: Option<bool>,
    /// `visibility: hidden` — 자리는 차지하되 안 보인다.
    pub hidden: Option<bool>,
    // 칠
    pub bg: Option<Token>,
    /// 진행률/스피너 같은 악센트 색.
    pub fill: Option<Token>,
    pub border_width: Option<f32>,
    pub border_color: Option<Token>,
    pub radius: Option<f32>,
    pub shadow: Option<Shadow>,
    pub shadow_color: Option<Token>,
    pub opacity: Option<f32>,
    // 글자
    pub color: Option<Token>,
    pub font_size: Option<f32>,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub mono: Option<bool>,
    pub strike: Option<bool>,
    pub underline: Option<bool>,
    pub text_align: Option<Align>,
    pub transform: Option<Transform>,
    pub truncate: Option<bool>,
    // 상호작용
    pub cursor: Option<Cursor>,
}

impl StyleSpec {
    /// 아무것도 선언하지 않은 값.
    pub const NONE: StyleSpec = StyleSpec {
        gap: None,
        row_gap: None,
        column_gap: None,
        padding: None,
        margin: None,
        width: None,
        height: None,
        min_width: None,
        min_height: None,
        max_width: None,
        max_height: None,
        align: None,
        justify: None,
        wrap: None,
        display_none: None,
        hidden: None,
        bg: None,
        fill: None,
        border_width: None,
        border_color: None,
        radius: None,
        shadow: None,
        shadow_color: None,
        opacity: None,
        color: None,
        font_size: None,
        line_height: None,
        letter_spacing: None,
        bold: None,
        italic: None,
        mono: None,
        strike: None,
        underline: None,
        text_align: None,
        transform: None,
        truncate: None,
        cursor: None,
    };
}

impl Default for StyleSpec {
    fn default() -> Self {
        Self::NONE
    }
}

/// `Option<T>` 필드를 그대로 옮긴다 — `apply` / `inherit_from`의 반복 제거.
macro_rules! take {
    ($out:expr, $src:expr, $( $f:ident ),* $(,)?) => {
        $(
            if let Some(v) = $src.$f {
                $out.$f = Some(v);
            }
        )*
    };
}

/// 해석이 끝난 확정 스타일 (플랫폼 독립) — 어터가 그대로 다.
///
/// 색은 이미 팔레트로 확정되어 있어 어터가 팔레트를 몰라도 된다.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResolvedStyle {
    // 레이아웃
    pub gap: Option<f32>,
    pub row_gap: Option<f32>,
    pub column_gap: Option<f32>,
    pub padding: Option<Edges>,
    pub margin: Option<Edges>,
    pub width: Option<Len>,
    pub height: Option<Len>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub align: Option<Align>,
    pub justify: Option<Align>,
    pub wrap: Option<bool>,
    pub display_none: Option<bool>,
    pub hidden: Option<bool>,
    // 칠
    pub bg: Option<Color>,
    pub fill: Option<Color>,
    pub border_width: Option<f32>,
    pub border_color: Option<Color>,
    pub radius: Option<f32>,
    pub shadow: Option<Shadow>,
    pub shadow_color: Option<Color>,
    pub opacity: Option<f32>,
    // 글자
    pub color: Option<Color>,
    pub font_size: Option<f32>,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub mono: Option<bool>,
    pub strike: Option<bool>,
    pub underline: Option<bool>,
    pub text_align: Option<Align>,
    pub transform: Option<Transform>,
    pub truncate: Option<bool>,
    // 상호작용
    pub cursor: Option<Cursor>,
}

impl ResolvedStyle {
    /// 선언이 하나도 없으면 true.
    pub fn is_empty(&self) -> bool {
        *self == ResolvedStyle::default()
    }

    /// 한 터의 선언을 다 — **나중에 적용된 것이 이긴다** (스케이드).
    pub fn apply(&mut self, spec: &StyleSpec, palette: &Palette) {
        take!(
            self,
            spec,
            gap,
            row_gap,
            column_gap,
            padding,
            margin,
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
            align,
            justify,
            wrap,
            display_none,
            hidden,
            border_width,
            radius,
            shadow,
            opacity,
            font_size,
            line_height,
            letter_spacing,
            bold,
            italic,
            mono,
            strike,
            underline,
            text_align,
            transform,
            truncate,
            cursor,
        );
        // 색만 팔레트로 확정한다
        if let Some(t) = spec.bg {
            self.bg = Some(palette.get(t));
        }
        if let Some(t) = spec.fill {
            self.fill = Some(palette.get(t));
        }
        if let Some(t) = spec.border_color {
            self.border_color = Some(palette.get(t));
        }
        if let Some(t) = spec.shadow_color {
            self.shadow_color = Some(palette.get(t));
        }
        if let Some(t) = spec.color {
            self.color = Some(palette.get(t));
        }
    }

    /// 부모에서 **상속되는** 속성만 물려받는다 (CSS 상속 규칙).
    ///
    /// 상속되는 것: `color` `font-size` `line-height` `letter-spacing`
    /// `weight` `font-style` `font-family` `text-decoration` `text-align`
    /// `text-transform`.
    ///
    /// 상속되지 않는 것: 여백·크기·배경·테두리·그림자·정렬·커서 등.
    pub fn inherit_from(&mut self, parent: &ResolvedStyle) {
        take!(
            self,
            parent,
            color,
            font_size,
            line_height,
            letter_spacing,
            bold,
            italic,
            mono,
            strike,
            underline,
            text_align,
            transform,
        );
    }

    /// `display: none`인가 (그리지 않는다).
    pub fn is_display_none(&self) -> bool {
        self.display_none == Some(true)
    }

    /// `Row`가 가로로 쓸 간격 (`column-gap` > `gap`).
    pub fn column_gap(&self) -> Option<f32> {
        self.column_gap.or(self.gap)
    }

    /// `Col`이 세로로 쓸 간격 (`row-gap` > `gap`).
    pub fn row_gap(&self) -> Option<f32> {
        self.row_gap.or(self.gap)
    }

    /// 그림자를 플랫폼 값으로 옮길 때 쓸 색 (기본: 팔레트 `shadow`).
    pub fn shadow_color_or(&self, palette: &Palette) -> Color {
        self.shadow_color
            .unwrap_or_else(|| palette.get(Token::Shadow))
    }
}

// ── 등록부 ───────────────────────────────────────────────────

/// 등록된 렉터 하나 (선언 텍스트 조회·직렬화용).
#[derive(Clone, Debug, PartialEq)]
pub struct StyleProps {
    selector: String,
    spec: StyleSpec,
}

impl StyleProps {
    /// 사용자가 쓴 그대로의 셀렉터 (`.card` / `button:hover` / `*`).
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// 선언된 스타일 (토큰 그대로).
    pub fn spec(&self) -> &StyleSpec {
        &self.spec
    }

    /// 파싱된 셀렉터 (명시도·매칭에 쓴다).
    pub fn parsed(&self) -> Option<Selector> {
        Selector::parse(&self.selector)
    }

    /// 선언들을 **텍스트**로 돌려준다 (디버깅·직렬화).
    pub fn pairs(&self) -> Vec<(String, String)> {
        let s = &self.spec;
        let mut out: Vec<(String, String)> = Vec::new();
        // 클로저 대신 매크로 — `out`을 가변 대여하지 않는다
        macro_rules! num {
            ($key:expr, $v:expr) => {
                if let Some(v) = $v {
                    out.push(($key.to_string(), format!("{v}")));
                }
            };
        }
        macro_rules! tok {
            ($key:expr, $v:expr) => {
                if let Some(v) = $v {
                    out.push(($key.to_string(), v.name().to_string()));
                }
            };
        }
        macro_rules! flag {
            ($key:expr, $v:expr) => {
                if let Some(v) = $v {
                    out.push((
                        $key.to_string(),
                        if v { "true" } else { "false" }.to_string(),
                    ));
                }
            };
        }
        // 레이아웃
        num!("gap", s.gap);
        num!("row-gap", s.row_gap);
        num!("column-gap", s.column_gap);
        if let Some(v) = s.padding {
            out.push(("padding".into(), v.text()));
        }
        if let Some(v) = s.margin {
            out.push(("margin".into(), v.text()));
        }
        if let Some(v) = &s.width {
            out.push(("width".into(), v.text()));
        }
        if let Some(v) = &s.height {
            out.push(("height".into(), v.text()));
        }
        num!("min-width", s.min_width);
        num!("min-height", s.min_height);
        num!("max-width", s.max_width);
        num!("max-height", s.max_height);
        if let Some(v) = s.align {
            out.push(("align".into(), v.text().to_string()));
        }
        if let Some(v) = s.justify {
            out.push(("justify".into(), v.text().to_string()));
        }
        flag!("wrap", s.wrap);
        if let Some(v) = s.display_none {
            out.push((
                "display".into(),
                if v { "none" } else { "flex" }.to_string(),
            ));
        }
        if let Some(v) = s.hidden {
            out.push((
                "visibility".into(),
                if v { "hidden" } else { "visible" }.to_string(),
            ));
        }
        // 칠
        tok!("bg", s.bg);
        tok!("fill", s.fill);
        num!("border-width", s.border_width);
        tok!("border-color", s.border_color);
        num!("radius", s.radius);
        if let Some(v) = &s.shadow {
            out.push(("shadow".into(), v.text()));
        }
        tok!("shadow-color", s.shadow_color);
        num!("opacity", s.opacity);
        // 글자
        tok!("color", s.color);
        num!("font-size", s.font_size);
        num!("line-height", s.line_height);
        num!("letter-spacing", s.letter_spacing);
        if let Some(v) = s.bold {
            out.push((
                "weight".into(),
                if v { "bold" } else { "normal" }.to_string(),
            ));
        }
        if let Some(v) = s.italic {
            out.push((
                "font-style".into(),
                if v { "italic" } else { "normal" }.to_string(),
            ));
        }
        if let Some(v) = s.mono {
            out.push((
                "font-family".into(),
                if v { "monospace" } else { "proportional" }.to_string(),
            ));
        }
        if let Some(v) = s.strike {
            out.push((
                "text-decoration".into(),
                if v { "line-through" } else { "none" }.to_string(),
            ));
        } else if let Some(v) = s.underline {
            out.push((
                "text-decoration".into(),
                if v { "underline" } else { "none" }.to_string(),
            ));
        }
        if let Some(v) = s.text_align {
            out.push(("text-align".into(), v.text().to_string()));
        }
        if let Some(v) = s.transform {
            out.push(("text-transform".into(), v.text().to_string()));
        }
        flag!("truncate", s.truncate);
        // 상호작용
        if let Some(v) = s.cursor {
            out.push(("cursor".into(), v.text().to_string()));
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

/// 등록된 규칙 하나.
#[derive(Clone, Debug)]
struct Rule {
    props: StyleProps,
    selector: Selector,
    /// 등록 순서 — 명시도가 같으면 **뒤가 이긴다**.
    order: usize,
}

#[derive(Default)]
struct Registry {
    rules: Vec<Rule>,
    by_text: HashMap<String, usize>,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry::default()))
}

/// `css!`가 등록한 항목을 **다시 적용**할 수 있도록 순서대로 기억한다 (0.7.4).
///
/// 시작 섹션(ctor)이 도는 플랫폼에서는 `register`가 불릴 때마다 쌓인다.
fn recorded() -> &'static Mutex<Vec<(String, StyleSpec)>> {
    static REC: OnceLock<Mutex<Vec<(String, StyleSpec)>>> = OnceLock::new();
    REC.get_or_init(|| Mutex::new(Vec::new()))
}

/// `css!`가 부르는 등록. 같은 렉터는 **먼저 등록된 것이 이긴다**.
///
/// 셀렉터 목록(`.a, .b`)은 여기서 나눠 따로 등록한다.
///
/// 파싱할 수 없는 셀렉터는 **panic**한다 — 0.6.0까지는 조용히 건너뛰어
/// "스타일이 안 먹는다"로만 보였다 (0.6.1). `css!`는 같은 문법을 컴파일타임에
/// 검사하므로(`macros::css::validate_selector`) 여기서 걸리는 것은
/// `register`를 직접 부른 경우뿐이다.
pub fn register(entries: Vec<(&str, StyleSpec)>) {
    // `init_styles()`가 다시 적용할 수 있게 기억한다 (0.7.4 — 리포트 버그 13).
    {
        let mut rec = recorded().lock().unwrap();
        for (text, spec) in &entries {
            rec.push((text.to_string(), *spec));
        }
    }
    apply(entries);
}

/// 항목을 레지스트리에 적용한다 (기록하지 않는다 — `register`/`init_styles` 공용).
fn apply(entries: Vec<(&str, StyleSpec)>) {
    let mut reg = registry().lock().unwrap();
    for (text, spec) in entries {
        for one in text.split(',') {
            let one = one.trim();
            if one.is_empty() || reg.by_text.contains_key(one) {
                continue;
            }
            let Some(selector) = Selector::parse(one) else {
                panic!(
                    "elm-magic: 셀렉터 `{one}`를 파싱할 수 없습니다. \
                     마디는 `태그` / `*` / `.클래스` / `:상태`의 조합이고, \
                     클래스 이름은 `.` 뒤에 와야 합니다"
                );
            };
            let order = reg.rules.len();
            reg.rules.push(Rule {
                props: StyleProps {
                    selector: one.to_string(),
                    spec,
                },
                selector,
                order,
            });
            reg.by_text.insert(one.to_string(), order);
        }
    }
}

/// 지금까지 등록된 `css!` 규칙을 **다시 적용**한다 (0.7.4 — 리포트 버그 13).
///
/// 시작 섹션(ctor)이 도는 플랫폼에서는 등록이 이미 끝나 있어 이 호출은
/// 멱등한 안전판이다. 시작 등록이 돌지 않은 플랫폼(예: wasm)에서는 기억된
/// 항목이 없어 아무 일도 하지 않는다 — 그 경우는 다른 메커니즘이 필요하다.
/// `main()`에서 한 줄 부르면 스타일이 사라지는 사고를 막을 수 있다.
pub fn init_styles() {
    let owned: Vec<(String, StyleSpec)> = {
        let rec = recorded().lock().unwrap();
        rec.clone()
    };
    let entries: Vec<(&str, StyleSpec)> =
        owned.iter().map(|(s, spec)| (s.as_str(), *spec)).collect();
    apply(entries);
}

/// 테스트 전용: 등록된 규칙을 비운다 (`init_styles`가 다시 채우는지 확인용).
///
/// 전역 레지스트리를 건드리므로 **단독 테스트 바이너리**에서만 쓴다.
#[doc(hidden)]
pub fn reset_for_tests() {
    let mut reg = registry().lock().unwrap();
    reg.rules.clear();
    reg.by_text.clear();
}

/// 렉터 스트 그대로의 조회 (`.card`, `button`, `.card Button`).
pub fn lookup(selector: &str) -> Option<StyleProps> {
    let reg = registry().lock().unwrap();
    let idx = *reg.by_text.get(selector.trim())?;
    Some(reg.rules[idx].props.clone())
}

/// 클래스 셀렉터 조회 (`"card"`, `".card"` 둘 다) — `Element::class`의 이름.
pub fn lookup_class(name: &str) -> Option<StyleProps> {
    let bare = name.strip_prefix('.').unwrap_or(name);
    lookup(&format!(".{bare}"))
}

/// 태그 셀렉터 조회 (`"button"`, 대소문자 무시) — `Element::tag()`의 이름.
pub fn lookup_tag(name: &str) -> Option<StyleProps> {
    let want = name.strip_prefix('.').unwrap_or(name).to_lowercase();
    let reg = registry().lock().unwrap();
    reg.rules
        .iter()
        .find(|r| r.selector.sole_tag() == Some(want.as_str()))
        .map(|r| r.props.clone())
}

/// 등록된 셀터 수 (테스트·디버깅용).
pub fn len() -> usize {
    registry().lock().unwrap().rules.len()
}

/// 등록된 렉터 목록 (등록 순서) — `ELM_MAGIC_DUMP`/디버깅용.
pub fn selectors() -> Vec<String> {
    registry()
        .lock()
        .unwrap()
        .rules
        .iter()
        .map(|r| r.props.selector.clone())
        .collect()
}

/// 매칭 경로로 **최종 스타일**을 만든다 (사양서 6.1~6.3).
///
/// - `path[0]` = 자기 자신, 이후 **조상** (가까운 순)
/// - 상속: `inherited`가 있으면 상속 속성을 먼저 물려받는다
/// - 스케이드: **명시도 → 등록 순서**
pub fn resolve_nodes(
    path: &[Node],
    state: State,
    inherited: Option<&ResolvedStyle>,
    palette: &Palette,
) -> ResolvedStyle {
    let mut out = ResolvedStyle::default();
    if let Some(parent) = inherited {
        out.inherit_from(parent);
    }
    let reg = registry().lock().unwrap();
    let mut hits: Vec<&Rule> = reg
        .rules
        .iter()
        .filter(|r| r.selector.matches(path, state))
        .collect();
    hits.sort_by_key(|r| (r.selector.specificity(), r.order));
    for rule in hits {
        out.apply(&rule.props.spec, palette);
    }
    out
}

/// 조상 정보 없는 간편 해석 (클래스 목록 + 태그).
///
/// 후손/자식 셀터는 **조상이 없으므로 매칭될 수 없다** — 전체 해석은
/// `resolve_nodes`를 쓴다.
pub fn resolve(classes: &[String], tag: &str, palette: &Palette) -> ResolvedStyle {
    resolve_nodes(&[Node::new(tag, classes)], State::NONE, None, palette)
}

// ─ `class={…}` (사양서 6.2 — 조건부 스타일은 값이다) ────────

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

impl IntoClasses for Vec<String> {
    fn into_classes(self) -> Vec<String> {
        self
    }
}

impl IntoClasses for Vec<&str> {
    fn into_classes(self) -> Vec<String> {
        self.into_iter()
            .flat_map(IntoClasses::into_classes)
            .collect()
    }
}

impl<const N: usize> IntoClasses for [&str; N] {
    fn into_classes(self) -> Vec<String> {
        self.into_iter()
            .flat_map(IntoClasses::into_classes)
            .collect()
    }
}

impl<T: IntoClasses> IntoClasses for Option<T> {
    /// `class={maybe}` — `None`이면 클래스 없음.
    fn into_classes(self) -> Vec<String> {
        self.map(IntoClasses::into_classes).unwrap_or_default()
    }
}

impl<T: IntoClasses + Clone> IntoClasses for &T {
    fn into_classes(self) -> Vec<String> {
        self.clone().into_classes()
    }
}
