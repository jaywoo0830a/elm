//! gpui-kit 어댑터 (사양서 7.2) — elm-magic의 `css!`를 gpui-kit로.
//!
//! egui 어댑터와 **같은 계약**이다: 코어가 `ResolvedStyle`까지 해석하고, 이
//! 어댑터는 그 결과를 gpui-kit의 테마 토큰과 스타일 값으로 **옮기기만** 한다.
//!
//! - **팔레트**: elm-magic `Token` → gpui-kit `ThemeColor` ([`palette`]).
//!   그래서 `css! { .card { bg: surface; } }`가 활성 gpui-kit 테마를 따른다.
//! - **레이아웃**: `Col`/`Row` → `v_flex`/`h_flex`, `gap`/`padding`/`align`/`justify`.
//! - **글자**: `font-size` `weight` `italic` `underline` `text-align`
//!   `text-transform` `truncate` `color`.
//! - **칠**: `bg` `fill` `border` `radius` `shadow` `opacity`.
//! - **상호작용**: `on_click` / `on_change` / `on_enter`를 `cx.listener`로
//!   아레나에 전달하고 `cx.notify()`로 elm 프레임을 다시 돈다.
//!
//! [`ElmView<C>`]가 gpui `Render`를 구현하므로, 앱은
//! `cx.new(|cx| ElmView::new(cx))` 한 번만 연결하면 된다.
//!
//! ```ignore
//! use elm_magic_gpui::ElmView;
//!
//! struct App;
//! impl Render for App {
//!     fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         div().child(cx.new(|cx| ElmView::<Counter>::new(cx)))
//!     }
//! }
//! ```
//!
//! ## 탈출구 (`<Raw>`)
//!
//! gpui에서 `<Raw>` 클로저는 `&mut gpui_kit::Window`를 받는다. 요소를 돌려줄 수
//! 없으므로 키 바인딩 등록 같은 **부수 효과**용이다.

use std::any::Any;
use std::rc::Rc;

use elm_magic::style::{
    Align as StyleAlign, Color, Cursor, Len, Palette, ResolvedStyle, State, Token,
};
use elm_magic::{
    Arena, BannerEl, ButtonEl, CheckEl, ColEl, Element, FragmentEl, InputEl, ModalEl, ProgressEl,
    RawEl, RowEl, StrongEl, TabEl, TdEl, TextAreaEl, TextEl, ThEl, Widget,
};

use gpui_kit::base::{box_shadow, StyledExt as _};
use gpui_kit::component::{ActiveTheme as _, Theme};
use gpui_kit::prelude::{
    InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _,
};
use gpui_kit::{
    div, px, relative, AnyElement, ClickEvent, Context, CursorStyle, FocusHandle, Hsla,
    KeyDownEvent, Rgba, SharedString, TextAlign, Window,
};

/// 한 프레임이 그린 위젯 정보 (egui `Pass`와 같은 계약).
#[derive(Default)]
pub struct Pass {
    /// **스타일이 적용된** 엘리먼트: (태그, 확정 스타일).
    pub styles: Vec<(&'static str, ResolvedStyle)>,
    /// 그려진 버튼·탭·헤더 라벨 (순서대로).
    pub labeled: Vec<String>,
    /// 그려진 입력 필드 수.
    pub inputs: usize,
}

impl Pass {
    /// 태그의 스타일 (첫 번째) — 어댑터 검증용.
    pub fn style_of(&self, tag: &str) -> Option<&ResolvedStyle> {
        self.styles.iter().find(|(t, _)| *t == tag).map(|(_, s)| s)
    }
}

// ── 스타일 → gpui ───────────────────────────────────────────

/// elm-magic 색 → gpui 색.
fn hsla_of(c: Color) -> Hsla {
    Rgba {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: c.a as f32 / 255.0,
    }
    .into()
}

/// gpui 색 → elm-magic 색 (팔레트 구성용).
fn color_of(c: Hsla) -> Color {
    let r = c.to_rgb();
    Color::rgba(
        (r.r * 255.0).round() as u8,
        (r.g * 255.0).round() as u8,
        (r.b * 255.0).round() as u8,
        (r.a * 255.0).round() as u8,
    )
}

/// gpui-kit 테마 → elm-magic 팔레트 (사양서 6.3 — 테마는 팔레트다).
///
/// `css!`의 토큰 14종을 gpui-kit `ThemeColor`에 맞춘다. 활성 테마가 바뀌면
/// (라이트/다크/커스텀) `css!`로 쓴 색도 함께 따라온다.
pub fn palette(theme: &Theme) -> Palette {
    let c = &theme.colors;
    let mut p = Palette::dark();
    p.set(Token::Primary, color_of(c.primary));
    p.set(Token::OnPrimary, color_of(c.primary_foreground));
    // 카드 표면은 popover, 한 단계 낮은 표면은 muted (shadcn 관례).
    p.set(Token::Surface, color_of(c.popover));
    p.set(Token::SurfaceAlt, color_of(c.muted));
    p.set(Token::Background, color_of(c.background));
    p.set(Token::Text, color_of(c.foreground));
    p.set(Token::TextDim, color_of(c.muted_foreground));
    p.set(Token::Error, color_of(c.danger));
    p.set(Token::Warn, color_of(c.warning));
    p.set(Token::Success, color_of(c.success));
    p.set(Token::Info, color_of(c.info));
    p.set(Token::Border, color_of(c.border));
    // gpui-kit는 그림자 색을 전경색 10% 잉크로 쓴다.
    p.set(Token::Shadow, color_of(c.foreground.alpha(0.1)));
    p.set(Token::Overlay, color_of(c.overlay));
    p
}

/// `ResolvedStyle`을 gpui 스타일로 옮긴다 (해석은 코어가 끝냈다 — 매핑만).
fn apply<E: gpui_kit::Styled>(mut el: E, s: &ResolvedStyle) -> E {
    if let Some(bg) = s.bg {
        el = el.bg(hsla_of(bg));
    }
    if let Some(fill) = s.fill {
        el = el.bg(hsla_of(fill));
    }
    if let Some(w) = s.border_width {
        el = el.border(px(w));
    }
    if let Some(c) = s.border_color {
        el = el.border_color(hsla_of(c));
    }
    if let Some(r) = s.radius {
        el = el.rounded(px(r));
    }
    if let Some(sh) = s.shadow {
        let color = hsla_of(s.shadow_color.unwrap_or(Color::rgba(0, 0, 0, 25)));
        el = el.shadow(vec![box_shadow(
            px(sh.dx),
            px(sh.dy),
            px(sh.blur),
            px(sh.spread),
            color,
        )]);
    }
    if let Some(o) = s.opacity {
        el = el.opacity(o);
    }
    if let Some(e) = s.padding {
        el = el
            .pt(px(e.top))
            .pb(px(e.bottom))
            .pl(px(e.left))
            .pr(px(e.right));
    }
    if let Some(e) = s.margin {
        el = el
            .mt(px(e.top))
            .mb(px(e.bottom))
            .ml(px(e.left))
            .mr(px(e.right));
    }
    match s.width {
        Some(Len::Px(v)) => el = el.w(px(v)),
        Some(Len::Fill) => el = el.w_full(),
        _ => {}
    }
    match s.height {
        Some(Len::Px(v)) => el = el.h(px(v)),
        Some(Len::Fill) => el = el.h_full(),
        _ => {}
    }
    if let Some(v) = s.min_width {
        el = el.min_w(px(v));
    }
    if let Some(v) = s.max_width {
        el = el.max_w(px(v));
    }
    if let Some(v) = s.min_height {
        el = el.min_h(px(v));
    }
    if let Some(v) = s.max_height {
        el = el.max_h(px(v));
    }
    if let Some(g) = s.gap {
        el = el.gap(px(g));
    }
    if let Some(g) = s.row_gap {
        el = el.gap_y(px(g));
    }
    if let Some(g) = s.column_gap {
        el = el.gap_x(px(g));
    }
    if s.wrap == Some(true) {
        el = el.flex_wrap();
    }
    if let Some(c) = s.color {
        el = el.text_color(hsla_of(c));
    }
    if let Some(v) = s.font_size {
        el = el.text_size(px(v));
    }
    if let Some(v) = s.line_height {
        el = el.line_height(px(v));
    }
    if s.bold == Some(true) {
        el = el.font_bold();
    }
    if s.italic == Some(true) {
        el = el.italic();
    }
    if s.underline == Some(true) {
        el = el.underline();
    }
    if s.strike == Some(true) {
        el = el.line_through();
    }
    if let Some(a) = s.text_align {
        el = el.text_align(align_of(a));
    }
    if s.truncate == Some(true) {
        el = el.truncate();
    }
    if let Some(c) = s.cursor {
        el = el.cursor(cursor_of(c));
    }
    el
}

fn align_of(a: StyleAlign) -> TextAlign {
    match a {
        StyleAlign::Start => TextAlign::Left,
        StyleAlign::Center => TextAlign::Center,
        StyleAlign::End => TextAlign::Right,
    }
}

fn cursor_of(c: Cursor) -> CursorStyle {
    match c {
        Cursor::Default => CursorStyle::Arrow,
        Cursor::Pointer => CursorStyle::PointingHand,
        Cursor::Text => CursorStyle::IBeam,
        Cursor::Grab => CursorStyle::OpenHand,
        Cursor::Grabbing => CursorStyle::ClosedHand,
        Cursor::Move => CursorStyle::DragLink,
        Cursor::Crosshair => CursorStyle::Crosshair,
        Cursor::NotAllowed => CursorStyle::OperationNotAllowed,
        // gpui-kit에는 커서 숨김 변형이 없다 — 기본 화살표로 둔다.
        Cursor::None => CursorStyle::Arrow,
    }
}

/// `<Banner kind="…">`의 기본 색 토큰.
fn banner_token(kind: &str) -> Token {
    match kind {
        "error" => Token::Error,
        "warn" | "warning" => Token::Warn,
        "success" | "ok" => Token::Success,
        "info" => Token::Info,
        _ => Token::Primary,
    }
}

// ── ElmView — gpui Render ───────────────────────────────────

/// elm-magic 컴포넌트(`C`)를 gpui-kit로 그리는 뷰.
///
/// 헤드리스 `TestApp`과 달리 실제 gpui 창에서 `Render`된다. 상태 아레나는 이
/// 뷰가 소유하고, 이벤트는 `cx.listener`로 들어와 `cx.notify()`로 재렌더된다.
pub struct ElmView<C: elm_magic::Component + 'static> {
    props: C::Props,
    ctx: elm_magic::Ctx,
    pass: Pass,
    /// 입력 필드별로 안정적인 포커스 핸들 (인덱스 = 그려진 입력 순서).
    focus: Vec<FocusHandle>,
}

impl<C: elm_magic::Component + 'static> ElmView<C> {
    /// 기본 props로 만든다.
    pub fn new(cx: &mut Context<Self>) -> Self
    where
        C::Props: Default,
    {
        Self::with_props(C::Props::default(), cx)
    }

    /// props를 지정해 만든다.
    pub fn with_props(props: C::Props, _cx: &mut Context<Self>) -> Self {
        Self {
            props,
            ctx: elm_magic::Ctx::new(),
            pass: Pass::default(),
            focus: Vec::new(),
        }
    }

    /// 마지막 프레임이 그린 정보.
    pub fn pass(&self) -> &Pass {
        &self.pass
    }

    /// 상태 아레나 (테스트/이펙트 구동용).
    pub fn ctx(&self) -> &elm_magic::Ctx {
        &self.ctx
    }

    /// 상태 아레나 (가변).
    pub fn ctx_mut(&mut self) -> &mut elm_magic::Ctx {
        &mut self.ctx
    }
}

impl<C: elm_magic::Component + 'static> Render for ElmView<C> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 1) elm 프레임: 상태 → Element 트리
        let tree = elm_magic::frame::<C>(&mut self.ctx, &self.props);

        // 2) 활성 gpui-kit 테마 → elm 팔레트
        let theme = cx.theme().clone();
        let palette = palette(&theme);

        // 3) 트리 → gpui 요소 (+ 스타일/포커스 수집)
        let mut focus = std::mem::take(&mut self.focus);
        let mut pass = Pass::default();
        let el = {
            let mut builder = Builder {
                ancestors: Vec::new(),
                inherited: Vec::new(),
                palette: &palette,
                pass: &mut pass,
                focus: &mut focus,
                inputs: 0,
                node: 0,
                cx,
                window,
            };
            builder.build(&tree)
        };
        self.focus = focus;
        self.pass = pass;
        el
    }
}
// ── 트리 → gpui 요소 ────────────────────────────────────────

/// 순회 상태 — 조상 경로(후손/자식 셀렉터) · 상속 스택 · 노드 번호.
struct Builder<'a, 'b, 'c, C: elm_magic::Component + 'static> {
    /// 가까운 조상부터 (`ancestors[0]` = 부모).
    ancestors: Vec<&'a Element>,
    /// 상속 스택 (부모의 확정 스타일).
    inherited: Vec<ResolvedStyle>,
    palette: &'a Palette,
    pass: &'a mut Pass,
    focus: &'a mut Vec<FocusHandle>,
    /// 지금까지 그린 입력 필드 수.
    inputs: usize,
    /// 안정적인 요소 id용 노드 번호.
    node: usize,
    cx: &'b mut Context<'c, ElmView<C>>,
    window: &'b mut Window,
}

impl<'a, 'b, 'c, C: elm_magic::Component + 'static> Builder<'a, 'b, 'c, C> {
    /// 트리 안에서의 확정 스타일 — 후손/자식 셀렉터·상태·상속까지.
    fn style_for(&self, el: &Element, state: State) -> ResolvedStyle {
        el.resolved_style_in(&self.ancestors, state, self.inherited.last(), self.palette)
    }

    /// 노드마다 고유한 요소 id.
    fn id(&mut self) -> SharedString {
        self.node += 1;
        SharedString::from(format!("elm-magic-{}", self.node))
    }

    /// 노드 하나: 스타일 해석 → 기록 → 자식 재귀.
    fn build(&mut self, el: &'a Element) -> AnyElement {
        let disabled = el.is_disabled();
        let state = State::new(false, false, false, disabled);
        let mut style = self.style_for(el, state);

        // `<Banner>`는 색을 안 정하면 종류별 기본색을 쓴다.
        if let Element::Banner(BannerEl { kind, .. }) = el {
            if style.color.is_none() {
                style.color = Some(self.palette.get(banner_token(kind)));
            }
        }
        if !style.is_empty() {
            self.pass.styles.push((el.tag(), style.clone()));
        }
        // `display: none` — 자리도 차지하지 않는다.
        if style.is_display_none() {
            return div().into_any_element();
        }

        self.ancestors.insert(0, el);
        self.inherited.push(style.clone());
        let out = self.build_inner(el, &style);
        self.inherited.pop();
        self.ancestors.remove(0);
        out
    }

    /// 변형별 그리기.
    fn build_inner(&mut self, el: &'a Element, style: &ResolvedStyle) -> AnyElement {
        let palette = self.palette;
        match el {
            Element::Text(TextEl { text, .. })
            | Element::Strong(StrongEl { text, .. })
            | Element::Td(TdEl { text, .. }) => text_node(text, style),
            Element::Banner(BannerEl { text, .. }) => apply(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .child(text.clone()),
                style,
            )
            .into_any_element(),
            Element::Col(ColEl {
                children, on_click, ..
            }) => self.container(children, style, true, on_click.clone()),
            Element::Row(RowEl {
                children, on_click, ..
            }) => self.container(children, style, false, on_click.clone()),
            Element::Fragment(FragmentEl { children }) => {
                let mut d = div().flex().flex_col();
                for child in children {
                    d = d.child(self.build(child));
                }
                d.into_any_element()
            }
            Element::Button(ButtonEl {
                text,
                disabled,
                on_click,
                ..
            }) => self.button(text, *disabled, on_click.clone(), style),
            Element::Tab(TabEl {
                text,
                active,
                on_click,
                ..
            }) => self.tab(text, *active, on_click.clone(), style),
            Element::Th(ThEl { text, on_click, .. }) => self.th(text, on_click.clone(), style),
            Element::Check(CheckEl {
                checked,
                label,
                on_change,
                ..
            }) => self.check(*checked, label, on_change.clone(), style),
            Element::Input(InputEl {
                value,
                on_change,
                on_enter,
                ..
            }) => self.text_input(value, on_change.clone(), on_enter.clone(), style, false),
            Element::TextArea(TextAreaEl {
                value,
                on_change,
                on_enter,
                ..
            }) => self.text_input(value, on_change.clone(), on_enter.clone(), style, true),
            Element::Spinner(_) => apply(
                div()
                    .w(px(16.))
                    .h(px(16.))
                    .rounded_full()
                    .border(px(2.))
                    .border_color(hsla_of(palette.get(Token::Border))),
                style,
            )
            .into_any_element(),
            Element::Divider(_) => apply(
                div()
                    .w_full()
                    .h(px(1.))
                    .bg(hsla_of(palette.get(Token::Border))),
                style,
            )
            .into_any_element(),
            Element::Progress(ProgressEl { value, .. }) => self.progress(*value, style),
            Element::Modal(ModalEl {
                title, children, ..
            }) => self.modal(title, children, style),
            Element::Raw(RawEl { widget, .. }) => {
                // 유일한 탈출구 (사양서 7.3): gpui 창 핸들을 그대로 넘긴다.
                let payload: &mut dyn Any = &mut *self.window;
                widget.as_ref()(payload);
                div().into_any_element()
            }
        }
    }
}

// ── 위젯별 그리기 ───────────────────────────────────────────

impl<'a, 'b, 'c, C: elm_magic::Component + 'static> Builder<'a, 'b, 'c, C> {
    /// `Col`/`Row` 공통 — 레이아웃 + 자식 + 클릭.
    fn container(
        &mut self,
        children: &'a [Element],
        style: &ResolvedStyle,
        vertical: bool,
        on_click: Option<Rc<dyn Fn(&mut Arena)>>,
    ) -> AnyElement {
        let mut d = if vertical {
            div().v_flex()
        } else {
            div().h_flex()
        };
        match style.align {
            Some(StyleAlign::Start) => d = d.items_start(),
            Some(StyleAlign::Center) => d = d.items_center(),
            Some(StyleAlign::End) => d = d.items_end(),
            None => {}
        }
        match style.justify {
            Some(StyleAlign::Start) => d = d.justify_start(),
            Some(StyleAlign::Center) => d = d.justify_center(),
            Some(StyleAlign::End) => d = d.justify_end(),
            None => {}
        }
        for child in children {
            d = d.child(self.build(child));
        }
        let d = apply(d, style);
        if let Some(h) = on_click {
            let id = self.id();
            return d
                .id(id)
                .cursor(CursorStyle::PointingHand)
                .on_click(self.cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    h(&mut this.ctx.arena);
                    cx.notify();
                }))
                .into_any_element();
        }
        d.into_any_element()
    }

    /// `<Button>` — 클릭 가능한 라벨. `disabled`면 핸들러를 붙이지 않는다.
    fn button(
        &mut self,
        text: &str,
        disabled: bool,
        on_click: Option<Rc<dyn Fn(&mut Arena)>>,
        style: &ResolvedStyle,
    ) -> AnyElement {
        let mut d = div()
            .h_flex()
            .justify_center()
            .items_center()
            .child(text.to_string());
        if style.padding.is_none() {
            d = d.px(px(12.)).py(px(6.));
        }
        if style.radius.is_none() {
            d = d.rounded(px(6.));
        }
        if style.bg.is_none() {
            d = d.bg(hsla_of(self.palette.get(Token::SurfaceAlt)));
        }
        if style.color.is_none() {
            d = d.text_color(hsla_of(self.palette.get(Token::Text)));
        }
        let d = apply(d, style);
        self.pass.labeled.push(text.to_string());
        if disabled {
            return d.opacity(0.5).into_any_element();
        }
        if let Some(h) = on_click {
            let id = self.id();
            return d
                .id(id)
                .cursor(CursorStyle::PointingHand)
                .on_click(self.cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    h(&mut this.ctx.arena);
                    cx.notify();
                }))
                .into_any_element();
        }
        d.into_any_element()
    }

    /// `<Tab>` — 활성이면 표면색으로 강조.
    fn tab(
        &mut self,
        text: &str,
        active: bool,
        on_click: Option<Rc<dyn Fn(&mut Arena)>>,
        style: &ResolvedStyle,
    ) -> AnyElement {
        let mut d = div()
            .h_flex()
            .items_center()
            .px(px(10.))
            .py(px(6.))
            .child(text.to_string());
        if active {
            d = d
                .bg(hsla_of(self.palette.get(Token::SurfaceAlt)))
                .text_color(hsla_of(self.palette.get(Token::Text)));
        } else {
            d = d.text_color(hsla_of(self.palette.get(Token::TextDim)));
        }
        let d = apply(d, style);
        self.pass.labeled.push(text.to_string());
        if let Some(h) = on_click {
            let id = self.id();
            return d
                .id(id)
                .cursor(CursorStyle::PointingHand)
                .on_click(self.cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    h(&mut this.ctx.arena);
                    cx.notify();
                }))
                .into_any_element();
        }
        d.into_any_element()
    }

    /// `<Th>` — 표 헤더 셀 (굵게 + 클릭).
    fn th(
        &mut self,
        text: &str,
        on_click: Option<Rc<dyn Fn(&mut Arena)>>,
        style: &ResolvedStyle,
    ) -> AnyElement {
        let mut d = div().h_flex().items_center().child(text.to_string());
        if style.bold.is_none() {
            d = d.font_bold();
        }
        let d = apply(d, style);
        self.pass.labeled.push(text.to_string());
        if let Some(h) = on_click {
            let id = self.id();
            return d
                .id(id)
                .cursor(CursorStyle::PointingHand)
                .on_click(self.cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    h(&mut this.ctx.arena);
                    cx.notify();
                }))
                .into_any_element();
        }
        d.into_any_element()
    }

    /// `<Check>` — 클릭하면 반대 값으로 `on_change`.
    fn check(
        &mut self,
        checked: bool,
        label: &str,
        on_change: Option<Rc<dyn Fn(&mut Arena, bool)>>,
        style: &ResolvedStyle,
    ) -> AnyElement {
        let mark = if checked { "✓" } else { "" };
        let d = div()
            .h_flex()
            .items_center()
            .gap(px(6.))
            .child(mark.to_string())
            .child(label.to_string());
        let d = apply(d, style);
        if let Some(h) = on_change {
            let next = !checked;
            let id = self.id();
            return d
                .id(id)
                .cursor(CursorStyle::PointingHand)
                .on_click(self.cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    h(&mut this.ctx.arena, next);
                    cx.notify();
                }))
                .into_any_element();
        }
        d.into_any_element()
    }
}

impl<'a, 'b, 'c, C: elm_magic::Component + 'static> Builder<'a, 'b, 'c, C> {
    /// `<Input>` / `<TextArea>` — 포커스 가능한 필드.
    ///
    /// gpui의 텍스트 입력은 위젯 프로토콜이 아니라 창 포커스를 요구하므로,
    /// 노드마다 안정적인 [`FocusHandle`]을 잡아 두고 키 입력을 elm의
    /// `on_change` / `on_enter`로 옮긴다. (엔터는 `on_enter`, 백스페이스·스페이스
    /// 및 1글자 키는 편집.)
    fn text_input(
        &mut self,
        value: &str,
        on_change: Option<Rc<dyn Fn(&mut Arena, String)>>,
        on_enter: Option<Rc<dyn Fn(&mut Arena, String)>>,
        style: &ResolvedStyle,
        multiline: bool,
    ) -> AnyElement {
        let index = self.inputs;
        self.inputs += 1;
        while self.focus.len() <= index {
            let handle = self.cx.focus_handle();
            self.focus.push(handle);
        }
        let handle = self.focus[index].clone();
        self.pass.inputs += 1;

        let base = value.to_string();
        let mut d = div().h_flex().items_center().child(base.clone());
        if style.padding.is_none() {
            d = d.px(px(8.)).py(px(4.));
        }
        if style.radius.is_none() {
            d = d.rounded(px(4.));
        }
        if style.bg.is_none() {
            d = d.bg(hsla_of(self.palette.get(Token::Background)));
        }
        if style.border_width.is_none() {
            d = d
                .border(px(1.))
                .border_color(hsla_of(self.palette.get(Token::Border)));
        }
        if style.color.is_none() {
            d = d.text_color(hsla_of(self.palette.get(Token::Text)));
        }
        if multiline {
            d = d.min_h(px(48.));
        }
        let id = self.id();
        let focus_handle = handle.clone();
        let mut d = apply(d.id(id), style).track_focus(&handle);
        d = d.on_click(move |_e, window, cx| {
            window.focus(&focus_handle, cx);
        });
        d = d.on_key_down(self.cx.listener(move |this, ev: &KeyDownEvent, _w, cx| {
            let key = ev.keystroke.key.to_string();
            let mut next = base.clone();
            match key.as_str() {
                "backspace" => {
                    next.pop();
                }
                "enter" => {
                    if let Some(h) = &on_enter {
                        h(&mut this.ctx.arena, next.clone());
                    }
                    cx.notify();
                    return;
                }
                "space" => next.push(' '),
                _ => {
                    if key.chars().count() == 1 {
                        next.push_str(&key);
                    }
                }
            }
            if let Some(h) = &on_change {
                h(&mut this.ctx.arena, next);
            }
            cx.notify();
        }));
        d.into_any_element()
    }

    /// `<Progress value={0..1} />` — 트랙 + 채움 막대.
    fn progress(&mut self, value: f64, style: &ResolvedStyle) -> AnyElement {
        let height = match style.height {
            Some(Len::Px(v)) => v,
            _ => style.min_height.unwrap_or(8.0),
        };
        let frac = (value as f32).clamp(0.0, 1.0);
        let fill = style
            .fill
            .map(hsla_of)
            .unwrap_or_else(|| hsla_of(self.palette.get(Token::Primary)));
        let track_bg = style
            .bg
            .map(hsla_of)
            .unwrap_or_else(|| hsla_of(self.palette.get(Token::SurfaceAlt)));
        let bar = div().w(relative(frac)).h_full().bg(fill);
        let track = div()
            .w_full()
            .h(px(height))
            .bg(track_bg)
            .rounded(px(height / 2.0))
            .overflow_hidden()
            .child(bar);
        apply(track, style).into_any_element()
    }

    /// `<Modal>` — 제목 + 자식 패널 (오버레이는 플랫폼에 맡긴다).
    fn modal(
        &mut self,
        title: &'a str,
        children: &'a [Element],
        style: &ResolvedStyle,
    ) -> AnyElement {
        let mut panel = div()
            .v_flex()
            .gap(px(8.))
            .p(px(16.))
            .rounded(px(8.))
            .bg(hsla_of(self.palette.get(Token::Surface)));
        if !title.is_empty() {
            panel = panel.child(title.to_string());
        }
        for child in children {
            panel = panel.child(self.build(child));
        }
        apply(panel, style).into_any_element()
    }
}

/// 텍스트 노드 — `text-transform`을 렌더 직전에 적용한다.
fn text_node(text: &str, style: &ResolvedStyle) -> AnyElement {
    let mut content = text.to_string();
    if let Some(t) = style.transform {
        content = t.apply(&content);
    }
    apply(div().child(content), style).into_any_element()
}
