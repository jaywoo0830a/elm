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
    let tree = C::render(&mut ctx, &props);
    TestApp { ctx, props, tree }
}

impl<C: Component> TestApp<C> {
    fn rerender(&mut self) {
        self.ctx.base = 0;
        self.ctx.keys.clear();
        self.tree = C::render(&mut self.ctx, &self.props);
    }

    /// Run all pending effects spawned by `<-` (사양서 12: Cmd는 flush 전까지
    /// 실행 안 됨), re-rendering after each round until the queue drains.
    pub fn flush(&mut self) {
        for _ in 0..1000 {
            if self.ctx.arena.pending_count() == 0 {
                break;
            }
            let pending = self.ctx.arena.take_pending();
            for effect in pending {
                effect(&mut self.ctx.arena);
            }
            self.rerender();
        }
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

    /// Advance the simulated clock by `ms` and re-render — fires due
    /// `on_tick` intervals and `on_change(x) after <ms>` debounces.
    pub fn advance(&mut self, ms: u64) {
        self.ctx.now += ms;
        self.rerender();
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

    /// Click the first Button whose text matches.
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
        let mut out = Vec::new();
        collect_text(&self.tree, &mut out);
        out.join("\n")
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

/// Public helper: find a Button's on_click by text (used by tests/adapters).
pub fn find_button_text(el: &Element, text: &str) -> Option<H> {
    find_button(el, text)
}

fn find_button(el: &Element, text: &str) -> Option<H> {
    match el {
        Button { on_click, .. } if button_matches(el, text) => Some(on_click.clone()),
        Col { children, .. } | Row { children, .. } => {
            children.iter().find_map(|c| find_button(c, text))
        }
        _ => None,
    }
}

fn button_matches(el: &Element, text: &str) -> bool {
    match el {
        Button { text: t, .. } => t == text,
        _ => false,
    }
}

fn find_input_change(el: &Element) -> VH {
    match el {
        Input { on_change, .. } => on_change.clone(),
        Col { children, .. } | Row { children, .. } => {
            children.iter().find_map(find_input_change)
        }
        _ => None,
    }
}

fn find_input_enter(el: &Element) -> Option<(String, VH)> {
    match el {
        Input { value, on_enter, .. } => Some((value.clone(), on_enter.clone())),
        Col { children, .. } | Row { children, .. } => {
            children.iter().find_map(find_input_enter)
        }
        _ => None,
    }
}

fn collect_text(el: &Element, out: &mut Vec<String>) {
    match el {
        Text { text, .. } => out.push(text.clone()),
        Button { text, .. } => out.push(text.clone()),
        Input { value, .. } => out.push(format!("[input: {}]", value)),
        // `<Raw>` is ignored headless (사양서 7.3)
        Raw { .. } => {}
        Col { children, .. } | Row { children, .. } => {
            for c in children {
                collect_text(c, out);
            }
        }
    }
}

fn dump(el: &Element, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match el {
        Text { text, class } => {
            out.push_str(&format!("{}Text {:?}{}\n", pad, text, fmt_class(class)));
        }
        Button { text, disabled, class, .. } => {
            let d = if *disabled { " disabled" } else { "" };
            out.push_str(&format!(
                "{}Button {:?}{}\n",
                pad,
                text,
                format!("{}{}", d, fmt_class(class))
            ));
        }
        Input { value, class, .. } => {
            out.push_str(&format!(
                "{}Input value={:?}{}\n",
                pad,
                value,
                fmt_class(class)
            ));
        }
        Raw { class, .. } => {
            out.push_str(&format!("{}[raw]{}\n", pad, fmt_class(class)));
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
