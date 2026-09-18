//! Headless testing: mount, click, type, expect — no renderer, no runtime.
//!
//! v0.7: 탐색이 **위젯 프로토콜**(`Widget`) 위에서 이루어진다 — `Element`의
//! variant를 직접 매칭하지 않는다. 그래서 role 기반 셀렉터
//! (`click_role`, `sel!`), 접근성 트리(`a11y_tree`) 같은 새 도구가 자연스럽다.

use crate::element::{BoolHandler, Element, Handler};
use crate::state::Ctx;
use crate::widget::{Role, Widget};
use crate::Component;

/// A mounted component instance for headless tests.
pub struct TestApp<C: Component> {
    pub ctx: Ctx,
    pub props: C::Props,
    pub tree: Element,
}

/// Mount a component with default props.
pub fn mount<C: Component>() -> TestApp<C>
where
    C::Props: Default,
{
    mount_with::<C>(C::Props::default())
}

/// Mount a component with explicit props.
pub fn mount_with<C: Component>(props: C::Props) -> TestApp<C> {
    let mut ctx = Ctx::new();
    let tree = crate::frame::<C>(&mut ctx, &props);
    TestApp { ctx, props, tree }
}

// ─────────────────────────────────────────────────────────────
// Selector (v0.7) — role/tag/class/text의 선언적 조합
// ─────────────────────────────────────────────────────────────

/// 요소를 역할/태그/클래스/라벨로 찾는 셀렉터.
///
/// ```ignore
/// use elm_magic::sel;
/// app.click_sel(&sel!(role Button, "+"));
/// assert!(app.exists(&sel!(class card)));
/// assert!(app.exists(&sel!(tag input)));
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selector {
    role: Option<Role>,
    tag: Option<String>,
    class: Option<String>,
    text: Option<String>,
    subtree: Option<String>,
}

impl Selector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn role(role: Role) -> Self {
        Self {
            role: Some(role),
            ..Self::default()
        }
    }

    pub fn tag(tag: impl Into<String>) -> Self {
        Self {
            tag: Some(tag.into().to_lowercase()),
            ..Self::default()
        }
    }

    pub fn class(class: impl Into<String>) -> Self {
        Self {
            class: Some(class.into()),
            ..Self::default()
        }
    }

    /// 라벨(`Widget::label`)과 정확히 일치.
    pub fn label(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            ..Self::default()
        }
    }

    /// 서브트리 텍스트가 일치 (컨테이너 매칭).
    pub fn subtree(text: impl Into<String>) -> Self {
        Self {
            subtree: Some(text.into()),
            ..Self::default()
        }
    }

    // 조합 (빌더)
    pub fn and_role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }
    pub fn and_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into().to_lowercase());
        self
    }
    pub fn and_class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }
    pub fn and_label(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }
    pub fn and_subtree(mut self, text: impl Into<String>) -> Self {
        self.subtree = Some(text.into());
        self
    }

    /// 이 요소가 셀렉터에 맞는가 — 전부 프로토콜 질의다.
    pub fn matches(&self, el: &Element) -> bool {
        if let Some(role) = self.role {
            if el.role() != role {
                return false;
            }
        }
        if let Some(tag) = &self.tag {
            if !el.tag().eq_ignore_ascii_case(tag) {
                return false;
            }
        }
        if let Some(class) = &self.class {
            if !el.class().iter().any(|c| c == class) {
                return false;
            }
        }
        if let Some(text) = &self.text {
            if el.label() != Some(text.as_str()) {
                return false;
            }
        }
        if let Some(subtree) = &self.subtree {
            if el.subtree_text() != *subtree {
                return false;
            }
        }
        true
    }
}

impl<C: Component> TestApp<C> {
    fn rerender(&mut self) {
        self.tree = crate::frame::<C>(&mut self.ctx, &self.props);
    }

    /// `bus.emit(Name)` — 다음 프레임에서 `on_event` 핸들러가 실행된다 (사양서 5.3).
    pub fn emit(&mut self, name: &str) {
        self.ctx.arena.emit(name);
        self.rerender();
    }

    /// `navigate("/users/42")` — `on_navigate` 핸들러에 경로(String)가 전달된다.
    pub fn navigate(&mut self, path: &str) {
        self.ctx.arena.navigate_path(path);
        self.rerender();
    }

    /// 사용자 라우트 타입을 그대로 전달: `app.navigate_value(Route::User(42))`.
    pub fn navigate_value<T: 'static>(&mut self, value: T) {
        self.ctx.arena.navigate(value);
        self.rerender();
    }

    /// 온라인/오프라인 전환 — `on_net_change` 핸들러가 실행된다.
    pub fn set_online(&mut self, online: bool) {
        self.ctx.arena.set_online(online);
        self.rerender();
    }

    /// Register a stream mock (`mock_stream!(app, f, [..])`와 동일).
    pub fn set_stream_mock<T: 'static>(&self, name: &str, values: Vec<T>) {
        crate::runtime::set_stream_mock(name, values);
    }

    /// 살아있는 keyed 슬롯 수 — keyed 트리/unmount 검증용 (사양서 9.5).
    pub fn keyed_slot_count(&self) -> usize {
        self.ctx.arena.keyed_slot_count()
    }

    /// 아레나의 시계를 `ctx.now`에 맞춘다.
    fn sync_clock(&mut self) {
        self.ctx.arena.set_now(self.ctx.now);
    }

    /// due가 된 효과를 실행하고, 매 라운드 끝에 재렌더한다.
    fn run_due_effects(&mut self) {
        for _ in 0..1000 {
            let due = self.ctx.arena.take_due();
            if due.is_empty() {
                break;
            }
            for effect in due {
                effect(&mut self.ctx.arena);
            }
            self.rerender();
        }
    }

    /// Run all *due* effects spawned by `<-` (사양서 12: Cmd는 flush 전까지
    /// 실행 안 됨), re-rendering after each round until the queue drains.
    pub fn flush(&mut self) {
        self.sync_clock();
        self.run_due_effects();
    }

    /// 아직 due가 아닌(예약된) 효과가 남아 있는가?
    pub fn has_pending_after(&self) -> bool {
        self.ctx.arena.has_deferred()
    }

    /// Press a key: dispatch to the `on_key` handler registered by the
    /// last render (사양서 5.3).
    pub fn press_key(&mut self, key: &str) {
        let handler = self
            .ctx
            .keys
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, h)| h.clone())
            .unwrap_or_else(|| panic!("no on_key handler for {:?}", key));
        handler(&mut self.ctx.arena);
        self.rerender();
    }

    /// Advance the simulated clock by `ms`, re-render — fires due `on_tick`
    /// intervals and `on_change(x) after <ms>` debounces — then run effects
    /// whose due time has arrived (`<- f() after 300ms`).
    pub fn advance(&mut self, ms: u64) {
        self.ctx.now = self.ctx.now.saturating_add(ms);
        self.sync_clock();
        self.rerender();
        self.run_due_effects();
    }

    /// Pump one value from each live stream and re-render (사양서 5.4).
    pub fn pump(&mut self) {
        let tasks = self.ctx.arena.take_streams();
        let mut keep = Vec::new();
        for mut task in tasks {
            if task(&mut self.ctx.arena) {
                keep.push(task);
            }
        }
        self.ctx.arena.push_streams(keep);
        self.rerender();
    }

    /// Register a 1-arg effect mock (use `mock!(app, f, |a: T| ...)`).
    pub fn set_mock1<A: Clone + 'static, Out: 'static>(
        &self,
        name: &str,
        f: impl Fn(A) -> Out + 'static,
    ) {
        crate::runtime::set_mock1(name, f);
    }

    /// Register a 2-arg effect mock.
    pub fn set_mock2<A0: Clone + 'static, A1: Clone + 'static, Out: 'static>(
        &self,
        name: &str,
        f: impl Fn(A0, A1) -> Out + 'static,
    ) {
        crate::runtime::set_mock2(name, f);
    }

    /// Register a 3-arg effect mock.
    pub fn set_mock3<
        A0: Clone + 'static,
        A1: Clone + 'static,
        A2: Clone + 'static,
        Out: 'static,
    >(
        &self,
        name: &str,
        f: impl Fn(A0, A1, A2) -> Out + 'static,
    ) {
        crate::runtime::set_mock3(name, f);
    }

    /// `app.mock(search_api, |q| vec![..])` — 사양서 8.2의 `.mock(fn, impl)` 슈가.
    pub fn mock<F, Fut, A, Out>(self, _f: F, m: impl Fn(A) -> Out + 'static) -> Self
    where
        F: Fn(A) -> Fut,
        Fut: std::future::Future<Output = Out>,
        A: Clone + 'static,
        Out: 'static,
    {
        crate::runtime::set_mock1(&effect_name::<F>(), m);
        self
    }

    /// 2-인자 효과의 `.mock()`.
    pub fn mock2<F, Fut, A0, A1, Out>(self, _f: F, m: impl Fn(A0, A1) -> Out + 'static) -> Self
    where
        F: Fn(A0, A1) -> Fut,
        Fut: std::future::Future<Output = Out>,
        A0: Clone + 'static,
        A1: Clone + 'static,
        Out: 'static,
    {
        crate::runtime::set_mock2(&effect_name::<F>(), m);
        self
    }

    /// 3-인자 효과의 `.mock()`.
    pub fn mock3<F, Fut, A0, A1, A2, Out>(
        self,
        _f: F,
        m: impl Fn(A0, A1, A2) -> Out + 'static,
    ) -> Self
    where
        F: Fn(A0, A1, A2) -> Fut,
        Fut: std::future::Future<Output = Out>,
        A0: Clone + 'static,
        A1: Clone + 'static,
        A2: Clone + 'static,
        Out: 'static,
    {
        crate::runtime::set_mock3(&effect_name::<F>(), m);
        self
    }
}

impl<C: Component> TestApp<C> {
    /// Click the first interactive node whose text matches
    /// (`Button`/`Tab`/`Th` 라벨, 또는 컨테이너의 서브트리 텍스트).
    pub fn click(&mut self, text: &str) {
        let handler = find_click(&self.tree, text)
            .unwrap_or_else(|| panic!("no button with text {:?} in tree", text));
        handler(&mut self.ctx.arena);
        self.rerender();
    }

    /// role + 라벨로 클릭 (v0.7) — 라벨 텍스트가 바뀌어도 role이 같다.
    pub fn click_role(&mut self, role: Role, label: &str) {
        let handler = find_first(&self.tree, &|el| {
            el.role() == role && el.label() == Some(label) && el.on_click().is_some()
        })
        .and_then(|el| el.on_click().cloned())
        .unwrap_or_else(|| panic!("no {:?} labelled {:?} in tree", role, label));
        handler(&mut self.ctx.arena);
        self.rerender();
    }

    /// 셀렉터로 클릭 — `app.click_sel(&sel!(role Button, "+"))`.
    pub fn click_sel(&mut self, sel: &Selector) {
        let handler = find_first(&self.tree, &|el| sel.matches(el) && el.on_click().is_some())
            .and_then(|el| el.on_click().cloned())
            .unwrap_or_else(|| panic!("no element matching {:?} with on_click", sel));
        handler(&mut self.ctx.arena);
        self.rerender();
    }

    /// 셀렉터에 맞는 첫 요소 (`None`이면 없음).
    pub fn query(&self, sel: &Selector) -> Option<&Element> {
        find_first(&self.tree, &|el| sel.matches(el))
    }

    /// 셀렉터에 맞는 요소가 있는가?
    pub fn exists(&self, sel: &Selector) -> bool {
        self.query(sel).is_some()
    }

    /// Type into the first Input (fires `on_change`).
    pub fn type_(&mut self, value: &str) {
        let handler = find_first(&self.tree, &|el| el.on_value_change().is_some())
            .and_then(|el| el.on_value_change().cloned())
            .expect("no input with on_change in tree");
        handler(&mut self.ctx.arena, value.to_string());
        self.rerender();
    }

    /// Type into the Input/TextArea matching `selector` (class name or tag:
    /// `"input"`, `"textarea"`) — the 2-arg `type_` of 사양서 8.2.
    pub fn type_into(&mut self, selector: &str, value: &str) {
        let handler = find_first(&self.tree, &|el| {
            el.on_value_change().is_some() && input_matches(el, selector)
        })
        .and_then(|el| el.on_value_change().cloned())
        .unwrap_or_else(|| panic!("no input matching {:?} in tree", selector));
        handler(&mut self.ctx.arena, value.to_string());
        self.rerender();
    }

    /// Press Enter on the first Input (fires `on_enter` with its current value).
    pub fn press_enter(&mut self) {
        let (value, handler) = find_first(&self.tree, &|el| el.on_value_enter().is_some())
            .map(|el| {
                (
                    el.value().unwrap_or("").to_string(),
                    el.on_value_enter().cloned(),
                )
            })
            .unwrap_or_else(|| panic!("no input with on_enter in tree"));
        if let Some(h) = handler {
            h(&mut self.ctx.arena, value);
        }
        self.rerender();
    }

    /// Toggle the first `<Check>` whose label matches (fires `on_change`).
    pub fn toggle(&mut self, label: &str) {
        let (checked, handler) = find_check(&self.tree, label)
            .unwrap_or_else(|| panic!("no check with label {:?} in tree", label));
        if let Some(h) = handler {
            h(&mut self.ctx.arena, !checked);
        }
        self.rerender();
    }

    /// Set a `<Check>` explicitly (fires `on_change` only if the value changes).
    pub fn set_check(&mut self, label: &str, value: bool) {
        let (checked, handler) = find_check(&self.tree, label)
            .unwrap_or_else(|| panic!("no check with label {:?} in tree", label));
        if checked != value {
            if let Some(h) = handler {
                h(&mut self.ctx.arena, value);
            }
            self.rerender();
        }
    }

    /// Assert that the given text appears somewhere in the tree (사양서 8.2).
    pub fn assert_text(&self, expected: &str) {
        self.expect_text(expected);
    }

    /// Assert that the given text is rendered somewhere (사양서 8.2).
    pub fn assert_visible(&self, expected: &str) {
        self.expect_text(expected);
    }

    /// Assert that the given text is *not* rendered anywhere (사양서 8.2).
    pub fn assert_hidden(&self, unexpected: &str) {
        let all = self.text();
        assert!(
            !all.lines().any(|l| l.contains(unexpected)) && !all.contains(unexpected),
            "expected text {:?} to be hidden.\n--- tree text ---\n{}\n-----------------",
            unexpected,
            all
        );
    }

    /// Re-render the tree (platform loops / adapters call this after
    /// handlers mutate the arena).
    pub fn refresh(&mut self) {
        self.rerender();
    }

    /// The current element tree (for platform adapters / assertions).
    pub fn element(&self) -> &Element {
        &self.tree
    }

    /// All visible text, one node per line.
    pub fn text(&self) -> String {
        self.tree.texts().join("\n")
    }

    /// Assert that the given text appears somewhere in the tree.
    pub fn expect_text(&self, expected: &str) {
        let all = self.text();
        assert!(
            all.lines().any(|l| l.contains(expected)) || all.contains(expected),
            "expected text {:?} not found.\n--- tree text ---\n{}\n-----------------",
            expected,
            all
        );
    }

    /// Indented tree dump — the snapshot contract (사양서 8.3).
    pub fn render_tree(&self) -> String {
        let mut out = String::new();
        self.tree.dump(0, &mut out);
        out
    }

    /// 접근성 트리 (v0.7) — role + 라벨만 남긴 읽기 쉬운 덤프.
    ///
    /// ```ignore
    /// assert_eq!(app.a11y_tree(), "group\n  button \"+\"\n");
    /// ```
    pub fn a11y_tree(&self) -> String {
        let mut out = String::new();
        a11y(&self.tree, 0, &mut out);
        out
    }

    /// (role, label) 평탄 목록 (v0.7) — 특정 role이 화면에 있는지 검증용.
    pub fn roles(&self) -> Vec<(Role, Option<String>)> {
        let mut out = Vec::new();
        collect_roles(&self.tree, &mut out);
        out
    }

    /// 특정 role이 트리에 있는가?
    pub fn has_role(&self, role: Role) -> bool {
        self.roles().into_iter().any(|(r, _)| r == role)
    }
}

// ─────────────────────────────────────────────────────────────
// 프로토콜 기반 탐색 헬퍼 — variant 매칭 없음
// ─────────────────────────────────────────────────────────────

/// 함수 아이템의 타입 이름에서 함수명만 뽑는다 — `mock!(app, f, ..)`의
/// `stringify!(f)` 키와 같은 이름이 되어 두 방식이 한 레지스트리를 공유한다.
fn effect_name<T: ?Sized>() -> String {
    let full = std::any::type_name::<T>();
    let tail = full.rsplit("::").next().unwrap_or(full);
    tail.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Public helper: find a Button's on_click by text (used by tests/adapters).
pub fn find_button_text(el: &Element, text: &str) -> Option<Handler> {
    find_click(el, text)
}

/// 깊이 우선으로 조건을 만족하는 첫 요소를 찾는다.
fn find_first<'a>(el: &'a Element, pred: &dyn Fn(&Element) -> bool) -> Option<&'a Element> {
    if pred(el) {
        return Some(el);
    }
    for c in el.children().unwrap_or(&[]) {
        if let Some(found) = find_first(c, pred) {
            return Some(found);
        }
    }
    None
}

/// 클릭 핸들러 찾기: 리프(`Button`/`Tab`/`Th`)는 라벨 정확 일치,
/// 컨테이너(`on_click`이 있는 `Row`/`Col`)는 서브트리 텍스트 일치.
fn find_click(el: &Element, text: &str) -> Option<Handler> {
    if let Some(handler) = el.on_click() {
        let leaf = matches!(el.role(), Role::Button | Role::Tab | Role::ColumnHeader);
        let hit = if leaf {
            el.label() == Some(text)
        } else {
            el.subtree_text() == text
        };
        if hit {
            return Some(handler.clone());
        }
    }
    for c in el.children().unwrap_or(&[]) {
        if let Some(found) = find_click(c, text) {
            return Some(found);
        }
    }
    None
}

/// `type_into`의 셀렉터: 태그명(`input`/`textarea`) 또는 클래스명.
fn input_matches(el: &Element, selector: &str) -> bool {
    let tag_matches = (el.tag() == "input" && selector == "input")
        || (el.tag() == "textarea" && selector == "textarea");
    tag_matches || el.class().iter().any(|c| c == selector)
}

/// 체크박스 찾기 → (현재 값, 핸들러).
fn find_check(el: &Element, label: &str) -> Option<(bool, Option<BoolHandler>)> {
    if el.on_bool_change().is_some() && el.label() == Some(label) {
        return Some((el.checked().unwrap_or(false), el.on_bool_change().cloned()));
    }
    for c in el.children().unwrap_or(&[]) {
        if let Some(found) = find_check(c, label) {
            return Some(found);
        }
    }
    None
}

/// 접근성 트리 렌더.
fn a11y(el: &Element, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match el.label() {
        Some(label) => out.push_str(&format!("{}{} {:?}\n", pad, el.role(), label)),
        None => out.push_str(&format!("{}{}\n", pad, el.role())),
    }
    for c in el.children().unwrap_or(&[]) {
        a11y(c, depth + 1, out);
    }
}

fn collect_roles(el: &Element, out: &mut Vec<(Role, Option<String>)>) {
    out.push((el.role(), el.label().map(str::to_string)));
    for c in el.children().unwrap_or(&[]) {
        collect_roles(c, out);
    }
}
