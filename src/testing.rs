//! Headless testing: mount, click, type, expect — no renderer, no runtime.

use crate::element::Element;
use crate::state::Ctx;
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

    /// 아레나의 시계를 `ctx.now`에 맞춘다 — 효과의 `after <dur>`가 이 시계를 쓴다.
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
    ///
    /// `<- f() after 300ms`처럼 미래에 due인 효과는 실행되지 않는다 —
    /// 그건 `advance(ms)`가 시계를 전진시켜 실행한다.
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
    /// Exhausted streams are dropped.
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
    pub fn set_mock3<A0: Clone + 'static, A1: Clone + 'static, A2: Clone + 'static, Out: 'static>(
        &self,
        name: &str,
        f: impl Fn(A0, A1, A2) -> Out + 'static,
    ) {
        crate::runtime::set_mock3(name, f);
    }

    /// `app.mock(search_api, |q| vec![..])` — 사양서 8.2의 `.mock(fn, impl)` 슈가.
    ///
    /// 함수 아이템의 타입 이름에서 함수명을 얻어 `mock!`과 같은 레지스트리에
    /// 등록한다. `|q| ..`의 매개변수 타입은 함수 시그니처에서 추론된다.
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

    /// Click the first Button (or Tab/Th) whose text matches.
    pub fn click(&mut self, text: &str) {
        let handler = find_button(&self.tree, text)
            .unwrap_or_else(|| panic!("no button with text {:?} in tree", text));
        if let Some(h) = handler {
            h(&mut self.ctx.arena);
        }
        self.rerender();
    }

    /// Type into the first Input (fires `on_change`).
    pub fn type_(&mut self, value: &str) {
        let handler = find_input_change(&self.tree)
            .expect("no input with on_change in tree");
        handler(&mut self.ctx.arena, value.to_string());
        self.rerender();
    }
    /// Press Enter on the first Input (fires `on_enter` with its current value).
    pub fn press_enter(&mut self) {
        let (value, handler) = find_input_enter(&self.tree)
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

    /// Type into the Input/TextArea matching `selector` (class name or tag:
    /// `"input"`, `"textarea"`) — the 2-arg `type_` of 사양서 8.2.
    pub fn type_into(&mut self, selector: &str, value: &str) {
        let handler = find_input_change_in(&self.tree, selector)
            .unwrap_or_else(|| panic!("no input matching {:?} in tree", selector));
        handler(&mut self.ctx.arena, value.to_string());
        self.rerender();
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
            all.lines().any(|l| l.contains(expected))
                || all.contains(expected),
            "expected text {:?} not found.\n--- tree text ---\n{}\n-----------------",
            expected,
            all
        );
    }

    /// Indented tree dump — the snapshot contract (사양서 8.3).
    pub fn render_tree(&self) -> String {
        let mut out = String::new();
        dump(&self.tree, 0, &mut out);
        out
    }
}

use crate::element::Element::*;
use std::rc::Rc;

type H = Option<Rc<dyn Fn(&mut crate::state::Arena)>>;
type VH = Option<Rc<dyn Fn(&mut crate::state::Arena, String)>>;
type BH = Option<Rc<dyn Fn(&mut crate::state::Arena, bool)>>;

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
pub fn find_button_text(el: &Element, text: &str) -> Option<H> {
    find_button(el, text)
}

/// 클릭 가능한 요소의 핸들러를 텍스트로 찾는다.
///
/// - `Button` / `Tab` / `Th`: 라벨이 정확히 일치
/// - `Row` / `Col` / `Modal`(on_click): 서브트리 텍스트가 일치 (`<Row on_click>`)
fn find_button(el: &Element, text: &str) -> Option<H> {
    if clickable_text(el) == Some(text) {
        if let Button { on_click, .. } | Tab { on_click, .. } | Th { on_click, .. } = el {
            return Some(on_click.clone());
        }
    }
    match el {
        Row { on_click, .. } | Col { on_click, .. }
            if on_click.is_some() && el.subtree_text() == text =>
        {
            return Some(on_click.clone());
        }
        _ => {}
    }
    children_of(el)?.iter().find_map(|c| find_button(c, text))
}

fn clickable_text(el: &Element) -> Option<&str> {
    match el {
        Button { text, .. } | Tab { text, .. } | Th { text, .. } => Some(text),
        _ => None,
    }
}

fn children_of(el: &Element) -> Option<&[Element]> {
    el.children()
}

/// `type_into(selector, ..)`의 셀렉터: 태그명(`input`, `textarea`) 또는 클래스명.
fn selectable_input(el: &Element, selector: &str) -> bool {
    let tag_matches = matches!(
        (el, selector),
        (Input { .. }, "input") | (TextArea { .. }, "textarea")
    );
    tag_matches || el.class().iter().any(|c| c == selector)
}

fn find_input_change(el: &Element) -> VH {
    match el {
        Input { on_change, .. } | TextArea { on_change, .. } => on_change.clone(),
        _ => children_of(el)?.iter().find_map(find_input_change),
    }
}

fn find_input_change_in(el: &Element, selector: &str) -> VH {
    if matches!(el, Input { .. } | TextArea { .. }) && selectable_input(el, selector) {
        if let Input { on_change, .. } | TextArea { on_change, .. } = el {
            return on_change.clone();
        }
    }
    children_of(el)?.iter().find_map(|c| find_input_change_in(c, selector))
}

fn find_input_enter(el: &Element) -> Option<(String, VH)> {
    match el {
        Input { value, on_enter, .. } | TextArea { value, on_enter, .. } => {
            Some((value.clone(), on_enter.clone()))
        }
        _ => children_of(el)?.iter().find_map(find_input_enter),
    }
}

fn find_check(el: &Element, label: &str) -> Option<(bool, BH)> {
    match el {
        Check { checked, label: l, on_change, .. } if l == label => {
            Some((*checked, on_change.clone()))
        }
        _ => children_of(el)?.iter().find_map(|c| find_check(c, label)),
    }
}


fn dump(el: &Element, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match el {
        Text { text, class } => {
            out.push_str(&format!("{}Text {:?}{}\n", pad, text, fmt_class(class)));
        }
        Strong { text, class } => {
            out.push_str(&format!("{}Strong {:?}{}\n", pad, text, fmt_class(class)));
        }
        Banner { kind, text, class } => {
            out.push_str(&format!(
                "{}Banner kind={:?} {:?}{}\n",
                pad,
                kind,
                text,
                fmt_class(class)
            ));
        }
        Button { text, disabled, class, .. } => {
            let d = if *disabled { " disabled" } else { "" };
            out.push_str(&format!("{}Button {:?}{}{}\n", pad, text, d, fmt_class(class)));
        }
        Tab { text, active, class, .. } => {
            let a = if *active { " active" } else { "" };
            out.push_str(&format!("{}Tab {:?}{}{}\n", pad, text, a, fmt_class(class)));
        }
        Th { text, class, .. } => {
            out.push_str(&format!("{}Th {:?}{}\n", pad, text, fmt_class(class)));
        }
        Td { text, class } => {
            out.push_str(&format!("{}Td {:?}{}\n", pad, text, fmt_class(class)));
        }
        Input { value, class, .. } => {
            out.push_str(&format!(
                "{}Input value={:?}{}\n",
                pad,
                value,
                fmt_class(class)
            ));
        }
        TextArea { value, class, .. } => {
            out.push_str(&format!(
                "{}TextArea value={:?}{}\n",
                pad,
                value,
                fmt_class(class)
            ));
        }
        Check { checked, label, class, .. } => {
            out.push_str(&format!(
                "{}Check {:?} checked={}{}\n",
                pad,
                label,
                checked,
                fmt_class(class)
            ));
        }
        Spinner { class } => {
            out.push_str(&format!("{}Spinner{}\n", pad, fmt_class(class)));
        }
        Divider { class } => {
            out.push_str(&format!("{}Divider{}\n", pad, fmt_class(class)));
        }
        Progress { value, class } => {
            out.push_str(&format!("{}Progress {}{}\n", pad, value, fmt_class(class)));
        }
        Raw { class, .. } => {
            out.push_str(&format!("{}[raw]{}\n", pad, fmt_class(class)));
        }
        Modal { title, class, children, .. } => {
            out.push_str(&format!("{}Modal {:?}{}\n", pad, title, fmt_class(class)));
            for c in children {
                dump(c, depth + 1, out);
            }
        }
        Fragment { children } => {
            out.push_str(&format!("{}Fragment\n", pad));
            for c in children {
                dump(c, depth + 1, out);
            }
        }
        Col { children, class, .. } => {
            out.push_str(&format!("{}Col{}\n", pad, fmt_class(class)));
            for c in children {
                dump(c, depth + 1, out);
            }
        }
        Row { children, class, .. } => {
            out.push_str(&format!("{}Row{}\n", pad, fmt_class(class)));
            for c in children {
                dump(c, depth + 1, out);
            }
        }
    }
}

fn fmt_class(class: &[String]) -> String {
    if class.is_empty() {
        String::new()
    } else {
        format!(" .{}", class.join("."))
    }
}
