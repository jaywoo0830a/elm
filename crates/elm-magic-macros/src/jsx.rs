use proc_macro::{Delimiter, Group, Punct, Spacing, Span, TokenStream, TokenTree};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

pub struct Env<'a> {
    pub states: &'a HashSet<String>,
    /// next hidden slot index for lifecycle constructs (on_mount/on_tick/…)
    pub slots: &'a Cell<usize>,
    /// 콜백 prop (`on_select: fn(Id)`) 이름들 — 호출은 `__elm_cb_<name>.call(..)`.
    pub callbacks: &'a HashSet<String>,
    /// 본문이 `children`을 쓴다면 prop의 사전 바인딩(`__elm_children`)이 있다.
    pub has_children: bool,
    /// `#[store]` 인스턴스 이름(`app`) → 접근자/필드 정보.
    pub stores: &'a HashMap<String, crate::store::StoreInfo>,
}

impl<'a> Env<'a> {
    /// 상태/콜백/store가 없는 빈 환경 (`ui!`용).
    pub fn empty(
        states: &'a HashSet<String>,
        slots: &'a Cell<usize>,
        callbacks: &'a HashSet<String>,
        stores: &'a HashMap<String, crate::store::StoreInfo>,
    ) -> Self {
        Env {
            states,
            slots,
            callbacks,
            has_children: false,
            stores,
        }
    }
}

/// `app.theme` — store 필드 접근인가?
fn store_access(env: &Env, toks: &[TokenTree], i: usize) -> Option<(String, String)> {
    let id = match &toks[i] {
        TokenTree::Ident(id) => id.to_string(),
        _ => return None,
    };
    if env.states.contains(&id) {
        return None;
    }
    let info = env.stores.get(&id)?;
    match (toks.get(i + 1), toks.get(i + 2)) {
        (Some(TokenTree::Punct(p)), Some(TokenTree::Ident(f)))
            if p.as_char() == '.' && !is_field_access(toks, i) =>
        {
            let field = f.to_string();
            // 스토어에 없는 이름(`app.remember()`)은 store 접근이 아니다
            if !info.fields.iter().any(|x| *x == field) {
                return None;
            }
            Some((format!("__elm_store_{}", id), field))
        }
        _ => None,
    }
}

/// `bus.emit(Name)` / `emit(Name)` — 이벤트 버스 (사양서 5.3).
fn bus_emit(_env: &Env, toks: &[TokenTree], i: usize) -> Option<(String, usize)> {
    let after = match &toks[i] {
        TokenTree::Ident(id) if id.to_string() == "bus" => {
            match (toks.get(i + 1), toks.get(i + 2)) {
                (Some(TokenTree::Punct(p)), Some(TokenTree::Ident(m)))
                    if p.as_char() == '.' && m.to_string() == "emit" =>
                {
                    i + 3
                }
                _ => return None,
            }
        }
        TokenTree::Ident(id) if id.to_string() == "emit" => i + 1,
        _ => return None,
    };
    let g = match toks.get(after) {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => g,
        _ => return None,
    };
    let event: Vec<TokenTree> = g.stream().into_iter().collect();
    let src = TokenStream::from_iter(event).to_string();
    Some((format!("::core::stringify!({})", src), after + 1))
}

/// `net.is_online()` — 플랫폼 연결 상태 (사양서 5.3).
fn net_is_online(toks: &[TokenTree], i: usize) -> Option<usize> {
    match (&toks[i], toks.get(i + 1), toks.get(i + 2), toks.get(i + 3)) {
        (
            TokenTree::Ident(a),
            Some(TokenTree::Punct(p)),
            Some(TokenTree::Ident(b)),
            Some(TokenTree::Group(g)),
        ) if a.to_string() == "net"
            && p.as_char() == '.'
            && b.to_string() == "is_online"
            && g.delimiter() == Delimiter::Parenthesis =>
        {
            Some(i + 4)
        }
        _ => None,
    }
}

/// 렌더/이벤트 공통 특수 폼: `net.is_online()` / `bus.emit(..)` / 콜백 prop 호출 /
/// `children` prop / store 필드 읽기 (사양서 4.2, 5.3, 3.1).
///
/// `arena_read`는 `&…arena` 표현식, `arena_mut`는 `&mut …arena` 표현식.
fn special_form(
    env: &Env,
    toks: &[TokenTree],
    i: usize,
    arena_read: &str,
    arena_mut: &str,
) -> Option<(TokenStream, usize)> {
    if let Some(n) = net_is_online(toks, i) {
        return Some((parse_ts(&format!("({}.online())", arena_read)), n));
    }
    if let Some((ev, n)) = bus_emit(env, toks, i) {
        return Some((
            parse_ts(&format!(
                "{{ {}.emit(&{}.replace(' ', \"\")); }}",
                arena_mut, ev
            )),
            n,
        ));
    }
    let name = match &toks[i] {
        TokenTree::Ident(id) => id.to_string(),
        _ => return None,
    };
    if env.callbacks.contains(&name) {
        if let Some(TokenTree::Group(g)) = toks.get(i + 1) {
            if g.delimiter() == Delimiter::Parenthesis {
                let args: Vec<TokenTree> = g.stream().into_iter().collect();
                let args_ts = transform_event_read(&args, env, None, Level::Expr).to_string();
                return Some((
                    parse_ts(&format!(
                        "{{ let __elm_cb = __elm_cb_{n}.clone(); __elm_cb.call({a}, {args}); }}",
                        n = name,
                        a = arena_mut,
                        args = args_ts
                    )),
                    i + 2,
                ));
            }
        }
    }
    if env.has_children && name == "children" && !env.states.contains("children") {
        return Some((parse_ts("(__elm_children_prop.clone())"), i + 1));
    }
    if let Some((inst, field)) = store_access(env, toks, i) {
        return Some((
            parse_ts(&format!("({}.{}.get({}).clone())", inst, field, arena_read)),
            i + 3,
        ));
    }
    None
}

/// `<... key={expr}>`가 있으면 키 식 문자열.
fn key_attr(attrs: &[(String, AttrVal)]) -> Option<String> {
    attrs.iter().find_map(|(k, v)| {
        if k != "key" {
            return None;
        }
        Some(match v {
            AttrVal::Lit(s) => format!("::std::string::String::from({:?})", s),
            AttrVal::Expr(e) => e.to_string(),
            AttrVal::Flag => "::std::string::String::from(\"key\")".to_string(),
        })
    })
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum Mode {
    Render,     // plain code inside render: reads go through `ctx`
    EventRead,  // event handler: reads go through the `_elm_a` arena param
    EventWrite, // event handler top level: assignments become slot writes
}

fn punct(c: char) -> TokenTree {
    let mut p = Punct::new(c, Spacing::Alone);
    p.set_span(Span::call_site());
    TokenTree::Punct(p)
}
fn parse_ts(s: &str) -> TokenStream {
    s.parse()
        .unwrap_or_else(|e| panic!("elm-magic internal: bad generated code {:?}: {:?}", s, e))
}

/// 이벤트 본문에서의 위치 — `Stmt`는 문장(대입·메서드 호출·효과),
/// `Expr`는 식 내부(읽기 전용)다.
#[derive(Clone, Copy, PartialEq)]
enum Level {
    Stmt,
    Expr,
}

/// Rust 키워드 — 구조체 리터럴 판정에서 경로 토큰과 구분하기 위해 쓴다.
fn is_control_keyword(s: &str) -> bool {
    matches!(
        s,
        "if" | "else"
            | "match"
            | "while"
            | "for"
            | "loop"
            | "in"
            | "return"
            | "let"
            | "mut"
            | "ref"
            | "fn"
            | "struct"
            | "enum"
            | "impl"
            | "unsafe"
            | "async"
            | "await"
            | "move"
            | "dyn"
            | "where"
            | "break"
            | "continue"
            | "as"
            | "const"
            | "static"
            | "extern"
            | "pub"
            | "use"
            | "mod"
            | "trait"
            | "type"
            | "crate"
            | "self"
            | "super"
            | "true"
            | "false"
    )
}

/// `.field` 접근인가? `..`(범위)와 `..=`는 제외한다 — 그래야 `(0..n)`의 `n`이
/// 상태 읽기로 치환된다.
fn is_field_access(toks: &[TokenTree], i: usize) -> bool {
    if i == 0 {
        return false;
    }
    match &toks[i - 1] {
        TokenTree::Punct(p) if p.as_char() == '.' => !matches!(
            toks.get(i - 2),
            Some(TokenTree::Punct(q)) if q.as_char() == '.'
        ),
        _ => false,
    }
}

/// 기간 토큰 → u64(ms) 식: `300` | `300ms` | `5min` | `2s` | `{expr}` | ident.
fn duration_expr(toks: &[TokenTree]) -> String {
    if toks.len() == 1 {
        if let TokenTree::Group(g) = &toks[0] {
            if g.delimiter() == Delimiter::Brace {
                return g.stream().to_string();
            }
        }
    }
    let compact: String = toks
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join("")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    for (unit, mul) in [("ms", 1u64), ("min", 60_000), ("s", 1_000), ("m", 60_000)] {
        if let Some(n) = compact.strip_suffix(unit) {
            if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
                let v: u64 = n.parse().unwrap_or(0);
                return format!("{}u64", v.saturating_mul(mul));
            }
        }
    }
    compact
}

/// 문장의 끝 위치와 종료자(`;` 또는 `,`). 이벤트 본문에서는 `,`도 문장
/// 구분자로 쓴다: `on_click={a = 1, b = 2}`.
fn stmt_end(toks: &[TokenTree], from: usize) -> (usize, Option<char>) {
    let mut k = from;
    while k < toks.len() {
        if let TokenTree::Punct(p) = &toks[k] {
            match p.as_char() {
                ';' => return (k, Some(';')),
                ',' => return (k, Some(',')),
                _ => {}
            }
        }
        k += 1;
    }
    (toks.len(), None)
}

/// `state.method(..)`가 문장 위치인가? (`;`/`,`/끝이 뒤따르면 문장)
fn stmt_position(toks: &[TokenTree], after: usize) -> bool {
    match toks.get(after) {
        None => true,
        Some(TokenTree::Punct(p)) => matches!(p.as_char(), ';' | ','),
        _ => false,
    }
}

/// 슬롯 읽기 식.
fn read_expr(name: &str, arena: &str) -> String {
    format!("(__elm_state_{}.get({}).clone())", name, arena)
}

/// 이 토큰들(중첩 포함)에 상태 읽기가 있는가? — 있으면 이벤트 핸들러 안에서
/// `_elm_a`를 쓰게 되므로, 인자를 미리 계산해 두어야 한다(아레나 이중 차용 방지).
fn uses_state(toks: &[TokenTree], env: &Env) -> bool {
    toks.iter().any(|t| match t {
        TokenTree::Ident(id) => env.states.contains(&id.to_string()),
        TokenTree::Group(g) => uses_state(&g.stream().into_iter().collect::<Vec<_>>(), env),
        _ => false,
    })
}

/// `Todo { text }`의 필드 키를 채운다 → `Todo { text: (read), }`.
///
/// 중괄호는 그대로 유지한다(구조체 리터럴의 중괄호가 곧 이 그룹이다).
/// `if flag { text }`, `else { text }` 같은 *블록*은 건드리지 않는다 —
/// 앞 토큰이 경로(`Todo`)이고 그 전이 식의 시작일 때만 축약으로 본다.
fn shorthand_group(toks: &[TokenTree], i: usize, env: &Env, arena: &str) -> Option<String> {
    if i == 0 {
        return None;
    }
    let g = match &toks[i] {
        TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => g,
        _ => return None,
    };
    let inner: Vec<TokenTree> = g.stream().into_iter().collect();
    if inner.len() != 1 {
        return None;
    }
    let name = match &inner[0] {
        TokenTree::Ident(id) => id.to_string(),
        _ => return None,
    };
    if !env.states.contains(&name) {
        return None;
    }
    match &toks[i - 1] {
        TokenTree::Ident(id) if !is_control_keyword(&id.to_string()) => {}
        _ => return None,
    }
    if i >= 2 {
        let ok = match &toks[i - 2] {
            TokenTree::Punct(p) => "(:,=;{[&|+-*/%!?<>".contains(p.as_char()),
            _ => false,
        };
        if !ok {
            return None;
        }
    }
    Some(format!("{{ {}: ({}), }}", name, read_expr(&name, arena)))
}

/// Count top-level commas (for building arg tuples).
fn count_top_level_commas(toks: &[TokenTree]) -> usize {
    let mut n = 0;
    let mut depth = 0i32;
    for t in toks {
        match t {
            TokenTree::Group(_) => {}
            TokenTree::Punct(p) => match p.as_char() {
                '<' => depth += 1,
                '>' => depth -= 1,
                ',' if depth == 0 => n += 1,
                _ => {}
            },
            _ => {}
        }
    }
    n + 1 // args = commas + 1 (0 commas → 1 arg; empty args handled by caller)
}

/// Convenience so both `Vec<TokenTree>` and `TokenStream` can `.push(tok)`.
/// (Vec's inherent `push` takes precedence.)
#[allow(dead_code)]
trait PushTok {
    fn push(&mut self, t: TokenTree);
}
impl PushTok for TokenStream {
    fn push(&mut self, t: TokenTree) {
        self.extend([t]);
    }
}

/// Substitute a read of a state variable at `i`; handles `.map(...)` sugar.
fn substitute_read(toks: &[TokenTree], i: usize, _env: &Env, arena: &str) -> (TokenStream, usize) {
    let name = match &toks[i] {
        TokenTree::Ident(id) => id.to_string(),
        _ => unreachable!(),
    };
    let state_var = format!("__elm_state_{}", name);
    // `items.map(...)` sugar → `(state).into_iter().map(...)`
    // 소유 반복이라 클로저가 아이템을 값으로 캡처한다 (사양서 3.3의
    // "`.iter()` / `&` 생략 가능" — 이벤트 핸들러에 `t`를 넘길 수 있어야 한다).
    if let (Some(TokenTree::Punct(dot)), Some(TokenTree::Ident(m))) =
        (toks.get(i + 1), toks.get(i + 2))
    {
        if dot.as_char() == '.' && m.to_string() == "map" {
            let out = parse_ts(&format!(
                "({}.get({}).clone()).into_iter().map",
                state_var, arena
            ));
            return (out, i + 3);
        }
        // `items.iter()` → 소유 반복 (`&T` 캡처로 인한 E0716 방지, 사양서 3.3)
        if dot.as_char() == '.' && m.to_string() == "iter" {
            // `.iter()`의 빈 괄호까지 소비한다
            let empty_parens = matches!(toks.get(i + 3),
                Some(TokenTree::Group(g))
                    if g.delimiter() == Delimiter::Parenthesis && g.stream().is_empty());
            let out = parse_ts(&format!(
                "({}.get({}).clone()).into_iter()",
                state_var, arena
            ));
            return (out, if empty_parens { i + 4 } else { i + 3 });
        }
    }
    let out = parse_ts(&format!("({}.get({}).clone())", state_var, arena));
    (out, i + 1)
}

/// 지역 컬렉션의 `.iter().map(..)` 슈가: `x.iter().map(..)` →
/// `(x).clone().into_iter().map(..)`.
///
/// 상태 슬롯은 [`substitute_read`]가 처리하므로 여기서는 **상태가 아닌**
/// 식별자(렌더 본문의 `let` 지역 변수)만 대상으로 한다. `&T`를 `'static`
/// 핸들러에 캡처할 수 없어(E0716) 소유 반복으로 바꾼다 (사양서 3.3).
///
/// `clone`을 거치는 이유: 지역 변수를 나중에 다시 쓸 수 있어야 하기 때문
/// (상태 슬롯 슈가가 `.clone()`을 하는 것과 같은 이유).
fn local_iter_sugar(toks: &[TokenTree], i: usize, env: &Env) -> Option<(TokenStream, usize)> {
    let name = match &toks[i] {
        TokenTree::Ident(id) => id.to_string(),
        _ => return None,
    };
    if env.states.contains(&name) || is_field_access(toks, i) {
        return None;
    }
    // `name . iter ( ) . map`
    match (
        toks.get(i + 1),
        toks.get(i + 2),
        toks.get(i + 3),
        toks.get(i + 4),
        toks.get(i + 5),
    ) {
        (
            Some(TokenTree::Punct(d1)),
            Some(TokenTree::Ident(m1)),
            Some(TokenTree::Group(g)),
            Some(TokenTree::Punct(d2)),
            Some(TokenTree::Ident(m2)),
        ) if d1.as_char() == '.'
            && m1.to_string() == "iter"
            && g.delimiter() == Delimiter::Parenthesis
            && g.stream().is_empty()
            && d2.as_char() == '.'
            && m2.to_string() == "map" =>
        {
            let out = parse_ts(&format!("({}).clone().into_iter().map", name));
            Some((out, i + 6))
        }
        _ => None,
    }
}

pub fn transform_top(toks: &[TokenTree], env: &Env) -> TokenStream {
    transform_children(toks, env, ';')
}

/// Children position: literals become Text, braces become into_elements,
/// `<Tag ...>` becomes elements, `let` statements pass through.
/// `sep` is the separator between expression pieces (`;` in a block body,
/// `,` inside a `vec![...]`).
fn transform_children(toks: &[TokenTree], env: &Env, sep: char) -> TokenStream {
    let mut out: Vec<TokenTree> = Vec::new();
    let mut pieces: Vec<(TokenStream, bool)> = Vec::new(); // (tokens, is_stmt)
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            TokenTree::Ident(id) if id.to_string() == "on_mount" => {
                if let Some(TokenTree::Group(g)) = toks.get(i + 1) {
                    if g.delimiter() == Delimiter::Brace {
                        let slot = env.slots.get();
                        env.slots.set(slot + 1);
                        let body: Vec<TokenTree> = g.stream().into_iter().collect();
                        let body_ts = transform_event(&body, env, None, Level::Stmt).to_string();
                        pieces.push((
                            parse_ts(&format!(
                                "{{ \
                                let __elm_m = __elm_ctx.slot({s}usize, || false); \
                                if !*__elm_m.get(&__elm_ctx.arena) {{ \
                                    __elm_m.set(&mut __elm_ctx.arena, true); \
                                    let _elm_a = &mut __elm_ctx.arena; \
                                    {body} \
                                }} \
                                }}",
                                s = slot,
                                body = body_ts
                            )),
                            false,
                        ));
                        i += 2;
                        continue;
                    }
                }
                panic!("elm-magic: on_mount expects a block: on_mount {{ ... }}");
            }
            TokenTree::Ident(id) if id.to_string() == "on_unmount" => {
                if let Some(TokenTree::Group(g)) = toks.get(i + 1) {
                    if g.delimiter() == Delimiter::Brace {
                        let body: Vec<TokenTree> = g.stream().into_iter().collect();
                        let body_ts = transform_event(&body, env, None, Level::Stmt).to_string();
                        pieces.push((
                            parse_ts(&format!(
                                "__elm_ctx.on_unmount(::std::rc::Rc::new(move |_elm_a: &mut ::elm_magic::Arena| {{ {} }}));",
                                body_ts
                            )),
                            false,
                        ));
                        i += 2;
                        continue;
                    }
                }
                panic!("elm-magic: on_unmount expects a block: on_unmount {{ ... }}");
            }
            TokenTree::Ident(id) if id.to_string() == "on_net_change" => {
                if let Some(TokenTree::Group(g)) = toks.get(i + 1) {
                    if g.delimiter() == Delimiter::Brace {
                        let body: Vec<TokenTree> = g.stream().into_iter().collect();
                        let body_ts = transform_event(&body, env, None, Level::Stmt).to_string();
                        pieces.push((
                            parse_ts(&format!(
                                "__elm_ctx.on_net_change(::std::rc::Rc::new(move |_elm_a: &mut ::elm_magic::Arena| {{ {} }}));",
                                body_ts
                            )),
                            false,
                        ));
                        i += 2;
                        continue;
                    }
                }
                panic!("elm-magic: on_net_change expects a block: on_net_change {{ ... }}");
            }
            TokenTree::Ident(id) if id.to_string() == "on_event" => {
                match (toks.get(i + 1), toks.get(i + 2)) {
                    (Some(TokenTree::Group(name)), Some(TokenTree::Group(g)))
                        if name.delimiter() == Delimiter::Parenthesis
                            && g.delimiter() == Delimiter::Brace =>
                    {
                        let ev: Vec<TokenTree> = name.stream().into_iter().collect();
                        let ev_src = TokenStream::from_iter(ev).to_string();
                        let body: Vec<TokenTree> = g.stream().into_iter().collect();
                        let body_ts = transform_event(&body, env, None, Level::Stmt).to_string();
                        pieces.push((
                            parse_ts(&format!(
                                "__elm_ctx.on_event(&::core::stringify!({}).replace(' ', \"\"), ::std::rc::Rc::new(move |_elm_a: &mut ::elm_magic::Arena| {{ {} }}));",
                                ev_src, body_ts
                            )),
                            false,
                        ));
                        i += 3;
                        continue;
                    }
                    _ => panic!("elm-magic: on_event expects on_event(Name) {{ ... }}"),
                }
            }
            TokenTree::Ident(id) if id.to_string() == "on_navigate" => {
                if let Some(TokenTree::Group(g)) = toks.get(i + 1) {
                    if g.delimiter() == Delimiter::Parenthesis {
                        let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                        match (inner.first(), inner.get(1)) {
                            (Some(TokenTree::Punct(p)), Some(TokenTree::Ident(var)))
                                if p.as_char() == '|' =>
                            {
                                // `|path| body` — 닫는 `|`가 있으면 건너뛴다
                                let mut start = 2;
                                if matches!(inner.get(start), Some(TokenTree::Punct(pp)) if pp.as_char() == '|')
                                {
                                    start += 1;
                                }
                                let body: Vec<TokenTree> = inner[start..].to_vec();
                                let body_ts =
                                    transform_event(&body, env, None, Level::Stmt).to_string();
                                pieces.push((
                                    parse_ts(&format!(
                                        "__elm_ctx.on_navigate(::std::rc::Rc::new(move |_elm_a: &mut ::elm_magic::Arena, __elm_nav: &dyn ::core::any::Any| {{ let {v} = ::elm_magic::nav_take(__elm_nav); {} }}));",
                                        body_ts,
                                        v = var.to_string()
                                    )),
                                    false,
                                ));
                                i += 2;
                                continue;
                            }
                            _ => panic!("elm-magic: on_navigate expects on_navigate(|path| ...)"),
                        }
                    }
                }
                panic!("elm-magic: on_navigate expects on_navigate(|path| ...)");
            }
            TokenTree::Ident(id) if id.to_string() == "on_message" => {
                // `on_message(소스, 상태) { 본문 }` → `소스 -> 상태 { 본문 }` (사양서 5.4)
                match (toks.get(i + 1), toks.get(i + 2)) {
                    (Some(TokenTree::Group(args)), Some(TokenTree::Group(g)))
                        if args.delimiter() == Delimiter::Parenthesis
                            && g.delimiter() == Delimiter::Brace =>
                    {
                        let inner: Vec<TokenTree> = args.stream().into_iter().collect();
                        let comma = inner
                            .iter()
                            .position(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ','));
                        let (src, slot) = match comma {
                            Some(c) if c + 1 < inner.len() => {
                                (inner[..c].to_vec(), inner[c + 1..].to_vec())
                            }
                            _ => panic!(
                                "elm-magic: on_message expects on_message(소스, 상태) {{ ... }}"
                            ),
                        };
                        let slot_name = match slot.as_slice() {
                            [TokenTree::Ident(t)] => t.to_string(),
                            _ => panic!(
                                "elm-magic: on_message expects on_message(소스, 상태) {{ ... }}"
                            ),
                        };
                        // 구독은 인스턴스당 한 번만 (매 렌더마다 스트림이 새로 생기지 않게)
                        let once = env.slots.get();
                        env.slots.set(once + 1);
                        let spawn = emit_stream(&src, &slot_name, g, env, "&mut __elm_ctx.arena");
                        pieces.push((
                            parse_ts(&format!(
                                "{{ let __elm_on = __elm_ctx.slot({i}usize, || false); \
                                  if !*__elm_on.get(&__elm_ctx.arena) {{ \
                                      __elm_on.set(&mut __elm_ctx.arena, true); {spawn} \
                                  }} }}",
                                i = once,
                                spawn = spawn
                            )),
                            false,
                        ));
                        i += 3;
                        continue;
                    }
                    _ => {
                        panic!("elm-magic: on_message expects on_message(소스, 상태) {{ ... }}")
                    }
                }
            }
            TokenTree::Ident(id) if id.to_string() == "on_key" => {
                match (toks.get(i + 1), toks.get(i + 2)) {
                    (Some(TokenTree::Group(key)), Some(TokenTree::Group(g)))
                        if key.delimiter() == Delimiter::Parenthesis
                            && g.delimiter() == Delimiter::Brace =>
                    {
                        let key_ts =
                            transform_render(&key.stream().into_iter().collect::<Vec<_>>(), env)
                                .to_string();
                        let body: Vec<TokenTree> = g.stream().into_iter().collect();
                        let body_ts = transform_event(&body, env, None, Level::Stmt).to_string();
                        pieces.push((
                            parse_ts(&format!(
                                "__elm_ctx.keys.push((::std::convert::Into::into({}), \
                                 ::std::rc::Rc::new(move |_elm_a: &mut ::elm_magic::Arena| {{ {} }})));",
                                key_ts, body_ts
                            )),
                            false,
                        ));
                        i += 3;
                        continue;
                    }
                    _ => panic!("elm-magic: on_key expects on_key(\"Key\") {{ ... }}"),
                }
            }
            TokenTree::Ident(id) if id.to_string() == "on_tick" => {
                match (toks.get(i + 1), toks.get(i + 2)) {
                    (Some(TokenTree::Group(p)), Some(TokenTree::Group(g)))
                        if p.delimiter() == Delimiter::Parenthesis
                            && g.delimiter() == Delimiter::Brace =>
                    {
                        let period = duration_expr(&p.stream().into_iter().collect::<Vec<_>>());
                        let slot = env.slots.get();
                        env.slots.set(slot + 1);
                        let body: Vec<TokenTree> = g.stream().into_iter().collect();
                        let body_ts = transform_event(&body, env, None, Level::Stmt).to_string();
                        pieces.push((
                            parse_ts(&format!(
                                "{{ \
                                let __elm_t = __elm_ctx.slot({s}usize, || 0u64); \
                                if __elm_ctx.now - *__elm_t.get(&__elm_ctx.arena) >= {p} {{ \
                                    __elm_t.set(&mut __elm_ctx.arena, __elm_ctx.now); \
                                    let _elm_a = &mut __elm_ctx.arena; \
                                    {body} \
                                }} \
                                }}",
                                s = slot,
                                p = period,
                                body = body_ts
                            )),
                            false,
                        ));
                        i += 3;
                        continue;
                    }
                    _ => panic!("elm-magic: on_tick expects on_tick(ms) {{ ... }}"),
                }
            }
            TokenTree::Ident(id) if id.to_string() == "on_change" => {
                // on_change(expr) { ... }            — 즉시(변경 감지)
                // on_change(expr) after 300ms { ... } — 디바운스 (사양서 5.2)
                let val = match toks.get(i + 1) {
                    Some(TokenTree::Group(v)) if v.delimiter() == Delimiter::Parenthesis => {
                        v.clone()
                    }
                    _ => panic!(
                        "elm-magic: on_change expects on_change(expr) [after <dur>] {{ ... }}"
                    ),
                };
                let mut next = i + 2;
                let mut delay = "0u64".to_string();
                if matches!(toks.get(next), Some(TokenTree::Ident(a)) if a.to_string() == "after") {
                    if let Some(d) = toks.get(next + 1) {
                        delay = duration_expr(&[d.clone()]);
                    }
                    next += 2;
                }
                let g = match toks.get(next) {
                    Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => g.clone(),
                    _ => panic!(
                        "elm-magic: on_change expects on_change(expr) [after <dur>] {{ ... }}"
                    ),
                };
                let val_ts = transform_render(&val.stream().into_iter().collect::<Vec<_>>(), env)
                    .to_string();
                let p_slot = env.slots.get();
                let s_slot = p_slot + 1;
                let f_slot = p_slot + 2;
                env.slots.set(p_slot + 3);
                let body: Vec<TokenTree> = g.stream().into_iter().collect();
                let body_ts = transform_event(&body, env, None, Level::Stmt).to_string();
                pieces.push((
                    parse_ts(&format!(
                        "{{ \
                        let __elm_v = ({v}); \
                        let __elm_p = __elm_ctx.slot({p}usize, || ::std::option::Option::None); \
                        let __elm_s = __elm_ctx.slot({s}usize, || 0u64); \
                        let __elm_f = __elm_ctx.slot({f}usize, || false); \
                        let mut __elm_pend = __elm_p.get(&__elm_ctx.arena).clone(); \
                        let mut __elm_since = *__elm_s.get(&__elm_ctx.arena); \
                        let mut __elm_fired = *__elm_f.get(&__elm_ctx.arena); \
                        if __elm_pend.as_ref() != ::std::option::Option::Some(&__elm_v) {{ \
                            __elm_pend = ::std::option::Option::Some(__elm_v.clone()); \
                            __elm_since = __elm_ctx.now; \
                            __elm_fired = false; \
                        }} \
                        __elm_p.set(&mut __elm_ctx.arena, __elm_pend.clone()); \
                        __elm_s.set(&mut __elm_ctx.arena, __elm_since); \
                        if !__elm_fired && __elm_ctx.now - __elm_since >= {d} {{ \
                            __elm_f.set(&mut __elm_ctx.arena, true); \
                            let _elm_a = &mut __elm_ctx.arena; \
                            {body} \
                        }} \
                        }}",
                        v = val_ts,
                        p = p_slot,
                        s = s_slot,
                        f = f_slot,
                        d = delay,
                        body = body_ts
                    )),
                    false,
                ));
                i = next + 1;
                continue;
            }
            TokenTree::Ident(id) if id.to_string() == "let" => {
                let mut stmt = Vec::new();
                while i < toks.len() {
                    stmt.push(toks[i].clone());
                    if let TokenTree::Punct(p) = &toks[i] {
                        if p.as_char() == ';' {
                            i += 1;
                            break;
                        }
                    }
                    i += 1;
                }
                // transform state reads inside the statement too
                pieces.push((transform_render(&stmt, env), true));
                continue;
            }
            TokenTree::Literal(l) => {
                let s = l.to_string();
                if s.starts_with('"') {
                    pieces.push((text_expr(&s[1..s.len() - 1], env), false));
                    i += 1;
                    continue;
                }
                pieces.push((TokenStream::from(toks[i].clone()), false));
                i += 1;
                continue;
            }
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                // `{if ..}` / `{match ..}`는 분기마다 타입이 달라도 되도록
                // 매크로가 `Vec<Element>`로 통일한다 (사양서 3.4).
                let expr = transform_element_expr(&inner, env);
                if sep == ',' {
                    // children builder applies into_elements itself
                    pieces.push((expr, false));
                } else {
                    pieces.push((
                        parse_ts(&format!(
                            "::elm_magic::IntoElements::into_elements({})",
                            expr
                        )),
                        false,
                    ));
                }
                i += 1;
                continue;
            }
            TokenTree::Punct(p) if p.as_char() == '<' => {
                if let Some(TokenTree::Ident(_)) = toks.get(i + 1) {
                    let (el, next) = parse_element(toks, i, env);
                    pieces.push((el, false));
                    i = next;
                    continue;
                }
                pieces.push((TokenStream::from(toks[i].clone()), false));
                i += 1;
                continue;
            }
            _ => {}
        }
        pieces.push((TokenStream::from(toks[i].clone()), false));
        i += 1;
    }
    let n = pieces.len();
    if sep == ',' {
        // children builder: each piece contributes via IntoElements so that
        // Text, nested elements and iterators all flatten uniformly
        let mut code = String::from(
            "{ let mut __elm_children = ::std::vec::Vec::<::elm_magic::Element>::new(); ",
        );
        for (piece, is_stmt) in pieces {
            if is_stmt {
                code.push_str(&piece.to_string());
                code.push(' ');
            } else {
                code.push_str(&format!(
                    "__elm_children.extend(::elm_magic::IntoElements::into_elements({})); ",
                    piece
                ));
            }
        }
        code.push_str("__elm_children }");
        return code.parse().expect("elm-magic internal: bad children code");
    }
    for (idx, (piece, is_stmt)) in pieces.into_iter().enumerate() {
        out.extend(piece);
        if !is_stmt && idx + 1 < n {
            out.push(punct(sep));
        }
    }
    out.into_iter().collect()
}

/// Generic position (inside expressions/closures): recursion + state substitution.
#[allow(dead_code)]
fn transform_tokens(toks: &[TokenTree], env: &Env, mode: Mode) -> TokenStream {
    match mode {
        Mode::EventWrite => transform_event(toks, env, None, Level::Stmt),
        Mode::EventRead => transform_event_read(toks, env, None, Level::Expr),
        Mode::Render => transform_render(toks, env),
    }
}

fn transform_render(toks: &[TokenTree], env: &Env) -> TokenStream {
    let mut out: Vec<TokenTree> = Vec::new();
    let mut i = 0;
    let mut prev_colon = false;
    while i < toks.len() {
        // `ui! { ... }` — unwrap and treat as element sequence
        if let (TokenTree::Ident(id), Some(TokenTree::Punct(b)), Some(TokenTree::Group(g))) =
            (&toks[i], toks.get(i + 1), toks.get(i + 2))
        {
            if id.to_string() == "ui" && b.as_char() == '!' && g.delimiter() == Delimiter::Brace {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                let body = transform_children(&inner, env, ';');
                out.push(TokenTree::Group(Group::new(Delimiter::Brace, body)));
                i += 3;
                continue;
            }
        }
        // interpolated string literal in element position: "x {y}" → Text
        if let TokenTree::Literal(l) = &toks[i] {
            let s = l.to_string();
            if s.starts_with('"') && s.contains('{') {
                out.extend(text_expr(&s[1..s.len() - 1], env));
                i += 1;
                continue;
            }
        }
        // 구조체 리터럴 축약 `Todo { text }` (중괄호 유지)
        if matches!(&toks[i], TokenTree::Group(g) if g.delimiter() == Delimiter::Brace) {
            if let Some(sub) = shorthand_group(toks, i, env, "&__elm_ctx.arena") {
                out.extend(parse_ts(&sub));
                i += 1;
                continue;
            }
        }
        // 특수 폼: store 필드 / 콜백 prop 호출 / children / bus / net (사양서 4.2, 5.3, 3.1)
        if matches!(&toks[i], TokenTree::Ident(_)) {
            if let Some((ts, next)) =
                special_form(env, toks, i, "&__elm_ctx.arena", "&mut __elm_ctx.arena")
            {
                out.extend(ts);
                i = next;
                continue;
            }
        }
        if let TokenTree::Ident(id) = &toks[i] {
            let name = id.to_string();
            // 지역 컬렉션의 `.iter().map(..)` 슈가 (E0716 방지)
            if let Some((ts, next)) = local_iter_sugar(toks, i, env) {
                out.extend(ts);
                i = next;
                continue;
            }
            // field key (`name:`) and field access (`.name`) stay verbatim
            let is_field_key = matches!(toks.get(i + 1), Some(TokenTree::Punct(p))
                if p.as_char() == ':' && p.spacing() == Spacing::Alone);
            if !is_field_access(toks, i) && !is_field_key && env.states.contains(&name) {
                let (sub, next) = substitute_read(toks, i, env, "&__elm_ctx.arena");
                out.extend(sub);
                i = next;
                continue;
            }
        }
        if let TokenTree::Punct(p) = &toks[i] {
            if p.as_char() == '<' && !prev_colon {
                if let Some(TokenTree::Ident(_)) = toks.get(i + 1) {
                    let (el, next) = parse_element(toks, i, env);
                    out.extend(el);
                    i = next;
                    prev_colon = false;
                    continue;
                }
            }
            prev_colon = p.as_char() == ':';
        }
        if let TokenTree::Group(g) = &toks[i] {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            let body = transform_render(&inner, env);
            out.push(TokenTree::Group(Group::new(g.delimiter(), body)));
            i += 1;
            continue;
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out.into_iter().collect()
}

// ── events ──────────────────────────────────────────────────

/// Find the next top-level `->` stream arrow at or after `from`, stopping at
/// `;`. Returns the index of the `-` token, or None. Only fires when the
/// arrow is followed by a state slot and a brace body.
fn find_stream_arrow(toks: &[TokenTree], from: usize, env: &Env) -> Option<usize> {
    let mut k = from;
    while k < toks.len() {
        match &toks[k] {
            TokenTree::Punct(p) if p.as_char() == ';' => return None,
            TokenTree::Punct(p) if p.as_char() == '-' && p.spacing() == Spacing::Joint => {
                if let (
                    Some(TokenTree::Punct(gt)),
                    Some(TokenTree::Ident(target)),
                    Some(TokenTree::Group(body)),
                ) = (toks.get(k + 1), toks.get(k + 2), toks.get(k + 3))
                {
                    if gt.as_char() == '>'
                        && body.delimiter() == Delimiter::Brace
                        && env.states.contains(&target.to_string())
                    {
                        return Some(k);
                    }
                }
            }
            _ => {}
        }
        k += 1;
    }
    None
}

/// `소스 -> 슬롯 { 본문 }` / `on_message(소스, 이름) { 본문 }` 전개 (사양서 5.4).
///
/// - `슬롯`이 상태면 스트림 값을 그 슬롯에 쓴다(본문은 새 값을 읽는다).
/// - 상태가 아니면 본문 안에서 그 이름으로 지역 바인딩한다
///   (`on_message(ws, m) { msgs.push(m); }`).
///
/// 소스가 단순 호출 `f(args)`나 식별자면 `mock_stream!`이 값을 가로챌 수 있다.
/// `spawn_arena`는 스폰을 실행하는 위치의 아레나 식 (`_elm_a` / `&mut __elm_ctx.arena`).
fn emit_stream(
    lhs: &[TokenTree],
    slot: &str,
    body_group: &Group,
    env: &Env,
    spawn_arena: &str,
) -> String {
    let body_toks: Vec<TokenTree> = body_group.stream().into_iter().collect();
    let body_ts = transform_event(&body_toks, env, None, Level::Stmt).to_string();
    let cont = if env.states.contains(slot) {
        format!(
            "move |_elm_a: &mut ::elm_magic::Arena, __elm_v| {{ __elm_state_{t}.set(_elm_a, __elm_v); {body} }}",
            t = slot,
            body = body_ts
        )
    } else {
        format!(
            "move |_elm_a: &mut ::elm_magic::Arena, __elm_v| {{ let {t} = __elm_v; {body} }}",
            t = slot,
            body = body_ts
        )
    };

    // 소스 식: 이벤트 안이면 `_elm_a`, 렌더 안이면 `__elm_ctx` 기준으로 상태를 읽는다.
    let read_ts = |toks: &[TokenTree]| -> String {
        if spawn_arena == "_elm_a" {
            transform_event_read(toks, env, None, Level::Expr).to_string()
        } else {
            transform_render(toks, env).to_string()
        }
    };

    match lhs {
        // `f(a, b)` → 인자를 미리 계산해 두고 목을 확인한다 (`mock_stream!`)
        [TokenTree::Ident(f), TokenTree::Group(g)]
            if g.delimiter() == Delimiter::Parenthesis && !env.states.contains(&f.to_string()) =>
        {
            let parts = split_top_commas(&g.stream().into_iter().collect::<Vec<_>>());
            let mut hoist = String::new();
            let mut args = String::new();
            for (k, p) in parts.iter().enumerate() {
                if k > 0 {
                    args.push_str(", ");
                }
                args.push_str(&format!("__elm_src_args.{}", k));
                hoist.push_str(&format!("{}, ", read_ts(p)));
            }
            format!(
                "{{ let __elm_src_args = ({hoist}); {a}.spawn_mockable_stream({name:?}, move || ::std::iter::IntoIterator::into_iter({f}({args})), {cont}); }}",
                hoist = hoist,
                a = spawn_arena,
                name = f.to_string(),
                f = f,
                args = args,
                cont = cont
            )
        }
        // `ws` → 그대로 사용 (`on_message(ws, m)`)
        [TokenTree::Ident(f)] if !env.states.contains(&f.to_string()) => format!(
            "{{ {a}.spawn_mockable_stream({name:?}, move || ::std::iter::IntoIterator::into_iter({f}), {cont}); }}",
            a = spawn_arena,
            name = f.to_string(),
            f = f,
            cont = cont
        ),
        // 그 밖의 식: 목 없이 즉시 평가
        _ => format!(
            "{{ {a}.spawn_stream(::std::iter::IntoIterator::into_iter({lhs}), {cont}); }}",
            a = spawn_arena,
            lhs = read_ts(lhs),
            cont = cont
        ),
    }
}

/// 최상위 콤마로 나눈다 (중첩 그룹은 그대로).
fn split_top_commas(toks: &[TokenTree]) -> Vec<Vec<TokenTree>> {
    let mut out = Vec::new();
    let mut cur: Vec<TokenTree> = Vec::new();
    for t in toks {
        if matches!(t, TokenTree::Punct(p) if p.as_char() == ',') {
            out.push(std::mem::take(&mut cur));
            continue;
        }
        cur.push(t.clone());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Try to parse an effect `a, b <- expr` starting at `i`.
/// Returns (targets, index of RHS start) if present.
fn try_parse_effect(toks: &[TokenTree], i: usize, env: &Env) -> Option<(Vec<String>, usize)> {
    let mut j = i;
    let mut targets = Vec::new();
    loop {
        match toks.get(j) {
            Some(TokenTree::Ident(id)) if env.states.contains(&id.to_string()) => {
                targets.push(id.to_string());
                j += 1;
                match toks.get(j) {
                    Some(TokenTree::Punct(p)) if p.as_char() == ',' => j += 1,
                    _ => break,
                }
            }
            _ => return None,
        }
    }
    if targets.is_empty() {
        return None;
    }
    let is_arrow = matches!(
        (toks.get(j), toks.get(j + 1)),
        (Some(TokenTree::Punct(l)), Some(TokenTree::Punct(r)))
            if l.as_char() == '<' && l.spacing() == Spacing::Joint && r.as_char() == '-'
    );
    if is_arrow {
        Some((targets, j + 2))
    } else {
        None
    }
}

/// Event handler: `n += 1`, `text = e`, `items.push(x)` → slot ops.
///
/// `level`이 `Stmt`면 문장(대입·메서드 호출·효과)을 슬롯 조작으로 바꾸고,
/// `Expr`면 식 안이므로 읽기만 치환한다 — `if user.is_empty()`나
/// `submit(name.clone(), ..)`의 `name.clone()`을 문장으로 오해하지 않기 위해서다.
fn transform_event(
    toks: &[TokenTree],
    env: &Env,
    value_binding: Option<&str>,
    level: Level,
) -> TokenStream {
    let mut out: Vec<TokenTree> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        // stream? `expr -> slot { body }` (사양서 5.4) — detected before the
        // LHS is processed so the expression is not emitted twice.
        if let Some(j) = find_stream_arrow(toks, i, env) {
            let lhs: Vec<TokenTree> = toks[i..j].to_vec();
            let tname = match &toks[j + 2] {
                TokenTree::Ident(t) => t.to_string(),
                _ => unreachable!(),
            };
            let body_group = match &toks[j + 3] {
                TokenTree::Group(g) => g.clone(),
                _ => unreachable!(),
            };
            out.extend(parse_ts(&emit_stream(
                &lhs,
                &tname,
                &body_group,
                env,
                "_elm_a",
            )));
            i = j + 4;
            if i < toks.len() {
                if let TokenTree::Punct(sp) = &toks[i] {
                    if sp.as_char() == ';' || sp.as_char() == ',' {
                        out.push(punct(';'));
                        i += 1;
                    }
                }
            }
            continue;
        }
        if let TokenTree::Ident(id) = &toks[i] {
            let name = id.to_string();
            if name == "_" {
                match value_binding {
                    Some(v) => {
                        out.extend(parse_ts(&format!("::elm_magic::clone_value(&{})", v)));
                        i += 1;
                        continue;
                    }
                    None => {
                        // `{_}`는 값 이벤트(on_change/on_enter)에서만 의미가 있다.
                        out.extend(parse_ts(
                            "::core::compile_error!(\"elm-magic: `_`는 on_change/on_enter 같은 값 이벤트에서만 쓸 수 있습니다 (on_click/on_close에는 전달되는 값이 없습니다)\")",
                        ));
                        i += 1;
                        continue;
                    }
                }
            }
            // store 필드 대입: `app.theme = …` (사양서 4.2)
            if level == Level::Stmt {
                if let Some((inst, field)) = store_access(env, toks, i) {
                    let var = format!("{}.{}", inst, field);
                    let after = i + 3;
                    let op = match (toks.get(after), toks.get(after + 1)) {
                        (Some(TokenTree::Punct(p)), _)
                            if p.as_char() == '=' && p.spacing() == Spacing::Alone =>
                        {
                            Some(None)
                        }
                        (Some(TokenTree::Punct(p1)), Some(TokenTree::Punct(p2)))
                            if p1.spacing() == Spacing::Joint
                                && "+-*/%&|^".contains(p1.as_char())
                                && p2.as_char() == '='
                                && p2.spacing() == Spacing::Alone =>
                        {
                            Some(Some(p1.as_char()))
                        }
                        _ => None,
                    };
                    if let Some(op) = op {
                        let rhs_start = if op.is_none() { after + 1 } else { after + 2 };
                        let (rhs_end, term) = stmt_end(toks, rhs_start);
                        let rhs: Vec<TokenTree> = toks[rhs_start..rhs_end].to_vec();
                        let rhs_ts =
                            transform_event_read(&rhs, env, value_binding, Level::Expr).to_string();
                        let emitted = match op {
                            None => format!(
                                "{{ let __elm_rhs = ({}); {}.set(_elm_a, __elm_rhs); }}",
                                rhs_ts, var
                            ),
                            Some(c) => format!(
                                "{{ let __elm_rhs = ({}); {}.mutate(_elm_a, |__v| *__v {}= __elm_rhs); }}",
                                rhs_ts, var, c
                            ),
                        };
                        out.extend(parse_ts(&emitted));
                        i = rhs_end;
                        if term.is_some() {
                            out.push(punct(';'));
                            i += 1;
                        }
                        continue;
                    }
                }
            }
            // 특수 폼 읽기: store / 콜백 prop / children / bus / net
            if let Some((ts, next)) = special_form(env, toks, i, "_elm_a", "_elm_a") {
                out.extend(ts);
                i = next;
                continue;
            }
            if env.states.contains(&name) {
                // effect? `a, b <- expr [after <dur>]` (사양서 5.1, 5.2)
                if level == Level::Stmt {
                    if let Some((targets, rhs_start)) = try_parse_effect(toks, i, env) {
                        let (mut rhs_end, _term) = stmt_end(toks, rhs_start);
                        // `after 300ms` 접미사 → 지연 효과 (advance(ms)가 실행)
                        let mut delay: Option<String> = None;
                        for k in rhs_start..rhs_end {
                            if matches!(&toks[k], TokenTree::Ident(id) if id.to_string() == "after")
                            {
                                let (d_end, _) = stmt_end(toks, k + 1);
                                delay = Some(duration_expr(&toks[k + 1..d_end]));
                                rhs_end = k;
                                break;
                            }
                        }
                        let rhs: Vec<TokenTree> = toks[rhs_start..rhs_end].to_vec();
                        let rhs_ts =
                            transform_event_read(&rhs, env, value_binding, Level::Expr).to_string();
                        // simple call `f(a, b)` → dispatch through the mock
                        // registry (사양서 8.2); other RHS forms run as-is
                        let fut_expr = match rhs.as_slice() {
                            [TokenTree::Ident(f), TokenTree::Group(g)]
                                if g.delimiter() == Delimiter::Parenthesis
                                    && !env.states.contains(&f.to_string()) =>
                            {
                                let args: Vec<TokenTree> = g.stream().into_iter().collect();
                                let mut parts: Vec<Vec<TokenTree>> = vec![Vec::new()];
                                for t in args {
                                    if matches!(&t, TokenTree::Punct(p) if p.as_char() == ',') {
                                        parts.push(Vec::new());
                                    } else {
                                        parts.last_mut().unwrap().push(t);
                                    }
                                }
                                let parts: Vec<Vec<TokenTree>> =
                                    parts.into_iter().filter(|p| !p.is_empty()).collect();
                                let n = parts.len();
                                let args_ts: Vec<String> = parts
                                    .iter()
                                    .map(|p| {
                                        transform_event_read(p, env, value_binding, Level::Expr)
                                            .to_string()
                                    })
                                    .collect();
                                let args_list = args_ts.join(", ");
                                let real_call = format!("{}({})", f, args_list);
                                let tuple = match n {
                                    0 => "()".to_string(),
                                    1 => format!("({},)", args_list),
                                    _ => format!("({})", args_list),
                                };
                                let any_args = (0..n)
                                    .map(|k| format!("&__elm_args.{} as &dyn ::std::any::Any", k))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!(
                                "{{ \
                                let __elm_args = {}; \
                                ::elm_magic::runtime::maybe_mock({:?}, {}, \
                                    move || ::elm_magic::runtime::call_mock({:?}, ::std::vec![{}])) \
                                }}",
                                tuple, f.to_string(), real_call, f.to_string(), any_args
                            )
                            }
                            _ => format!("({})", rhs_ts),
                        };
                        let mut sets = String::new();
                        for (n, t) in targets.iter().enumerate() {
                            let out = if targets.len() == 1 {
                                "__elm_out".to_string()
                            } else {
                                format!("__elm_out.{}", n)
                            };
                            sets.push_str(&format!(
                                "__elm_state_{}.set(__elm_a2, ({}).clone()); ",
                                t, out
                            ));
                        }
                        let spawn = match &delay {
                        Some(d) => format!(
                            "_elm_a.spawn_after({}, __elm_fut, move |__elm_a2: &mut ::elm_magic::Arena, __elm_out| {{ {} }})",
                            d, sets
                        ),
                        None => format!(
                            "_elm_a.spawn(__elm_fut, move |__elm_a2: &mut ::elm_magic::Arena, __elm_out| {{ {} }})",
                            sets
                        ),
                    };
                        let emitted = format!("{{ let __elm_fut = {}; {}; }}", fut_expr, spawn);
                        out.extend(parse_ts(&emitted));
                        let (end, term) = stmt_end(toks, rhs_start);
                        i = end;
                        if term.is_some() {
                            out.push(punct(';'));
                            i += 1;
                        }
                        continue;
                    }
                }
                // struct-literal field key `name:` stays verbatim
                let is_field_key = matches!(toks.get(i + 1), Some(TokenTree::Punct(p))
                    if p.as_char() == ':' && p.spacing() == Spacing::Alone);
                if is_field_key {
                    out.push(toks[i].clone());
                    i += 1;
                    continue;
                }
                let state_var = format!("__elm_state_{}", name);
                if let Some(TokenTree::Punct(p1)) = toks.get(i + 1) {
                    let c1 = p1.as_char();
                    let joint = p1.spacing() == Spacing::Joint;
                    let plain_assign = c1 == '=' && !joint;
                    let op_assign = joint
                        && "+-*/%&|^".contains(c1)
                        && matches!(toks.get(i + 2), Some(TokenTree::Punct(p2))
                            if p2.as_char() == '=' && p2.spacing() == Spacing::Alone);
                    if (plain_assign || op_assign) && level == Level::Stmt {
                        let rhs_start = if plain_assign { i + 2 } else { i + 3 };
                        let (rhs_end, term) = stmt_end(toks, rhs_start);
                        let rhs: Vec<TokenTree> = toks[rhs_start..rhs_end].to_vec();
                        let rhs_ts =
                            transform_event_read(&rhs, env, value_binding, Level::Expr).to_string();
                        // precompute the RHS before taking the mutable borrow
                        let emitted = if plain_assign {
                            format!(
                                "{{ let __elm_rhs = ({}); {}.set(_elm_a, __elm_rhs); }}",
                                rhs_ts, state_var
                            )
                        } else {
                            format!(
                                "{{ let __elm_rhs = ({}); {}.mutate(_elm_a, |__v| *__v {}= __elm_rhs); }}",
                                rhs_ts, state_var, c1
                            )
                        };
                        out.extend(parse_ts(&emitted));
                        i = rhs_end;
                        if term.is_some() {
                            out.push(punct(';'));
                            i += 1;
                        }
                        continue;
                    }
                    // 문장 위치의 메서드 호출: `items.push(x)` → 실제 슬롯 변경.
                    // (`submit(name.clone(), ..)`의 `name.clone()`처럼 식 안의
                    //  메서드는 읽기로 남긴다)
                    if c1 == '.' && level == Level::Stmt && stmt_position(toks, i + 4) {
                        if let (Some(TokenTree::Ident(m)), Some(TokenTree::Group(g))) =
                            (toks.get(i + 2), toks.get(i + 3))
                        {
                            if g.delimiter() == Delimiter::Parenthesis {
                                let args: Vec<TokenTree> = g.stream().into_iter().collect();
                                let args_ts =
                                    transform_event_read(&args, env, value_binding, Level::Expr)
                                        .to_string();
                                // precompute args before taking the mutable borrow
                                let n_args = if args.is_empty() {
                                    0
                                } else {
                                    count_top_level_commas(&args)
                                };
                                // `items.remove(t)` — 사양서 3.3의 "요소로 삭제".
                                // 정수 리터럴(`items.remove(0)`)은 Vec의 인덱스 삭제로 남긴다.
                                let by_element = m.to_string() == "remove"
                                    && n_args == 1
                                    && !matches!(args.first(),
                                        Some(TokenTree::Literal(l))
                                            if l.to_string().chars().all(|c| c.is_ascii_digit()));
                                if by_element {
                                    // 인자가 상태에서 온 값이면 소유(클론)로, 그 밖에는
                                    // 참조로 잡는다 — 리스트 아이템을 `Fn` 핸들러 안에서
                                    // move하지 않기 위해서다 (E0507 방지).
                                    let from_state = uses_state(&args, env);
                                    let emitted = if from_state {
                                        format!(
                                            "{{ let __elm_item = ({}); \
                                              {}.mutate(_elm_a, |__v| {{ __v.retain(|__x| *__x != __elm_item); }}); }}",
                                            args_ts, state_var
                                        )
                                    } else {
                                        format!(
                                            "{{ let __elm_item = &({}); \
                                              {}.mutate(_elm_a, |__v| {{ __v.retain(|__x| *__x != *__elm_item); }}); }}",
                                            args_ts, state_var
                                        )
                                    };
                                    out.extend(parse_ts(&emitted));
                                    i += 4;
                                    // 문장 종료자(`;`/`,`)를 소비한다 — 컴마 다중문용.
                                    // (소비하지 않으면 뒤 문장과의 사이에 `,`가 그대로 남아
                                    //  `expected expression, found ','`가 된다.)
                                    if let Some(TokenTree::Punct(p)) = toks.get(i) {
                                        if matches!(p.as_char(), ';' | ',') {
                                            out.push(punct(';'));
                                            i += 1;
                                        }
                                    }
                                    continue;
                                }
                                // args가 상태를 참조할 때만 미리 계산한다(아레나 이중 차용 방지).
                                // 그 밖에는 인라인으로 넘겨서, 캡처된 값이 `&`로 전달될 수 있게 한다.
                                let from_state = uses_state(&args, env);
                                let mut decl = String::new();
                                let mut call_args = String::new();
                                if n_args > 0 {
                                    if from_state {
                                        decl = format!("let __elm_args = ({},); ", args_ts);
                                        for k in 0..n_args {
                                            if k > 0 {
                                                call_args.push_str(", ");
                                            }
                                            call_args.push_str(&format!("__elm_args.{}", k));
                                        }
                                    } else {
                                        call_args = args_ts.clone();
                                    }
                                }
                                // 반환값은 버린다 (`items.remove(0)`처럼 값을 돌려주는
                                // 메서드도 문장으로 쓸 수 있게)
                                let emitted =
                                    format!(
                                    "{{ {} {}.mutate(_elm_a, |__v| {{ let _ = __v.{}({}); }}); }}",
                                    decl, state_var, m.to_string(), call_args
                                );
                                out.extend(parse_ts(&emitted));
                                i += 4;
                                // 문장 종료자(`;`/`,`)를 소비한다 — 컴마 다중문용
                                if let Some(TokenTree::Punct(p)) = toks.get(i) {
                                    if matches!(p.as_char(), ';' | ',') {
                                        out.push(punct(';'));
                                        i += 1;
                                    }
                                }
                                continue;
                            }
                        }
                    }
                }
                let (sub, next) = substitute_read(toks, i, env, "_elm_a");
                out.extend(sub);
                i = next;
                continue;
            }
        }
        if let TokenTree::Group(g) = &toks[i] {
            // 구조체 리터럴 축약 `Todo { text }` → `Todo { text: (read) }`
            if let Some(sub) = shorthand_group(toks, i, env, "_elm_a") {
                out.extend(parse_ts(&sub));
                i += 1;
                continue;
            }
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            // 괄호/대괄호 안은 식(Expr), 중괄호 블록은 레벨 유지
            let next_level = match g.delimiter() {
                Delimiter::Parenthesis | Delimiter::Bracket => Level::Expr,
                _ => level,
            };
            let body = transform_event(&inner, env, value_binding, next_level);
            out.push(TokenTree::Group(Group::new(g.delimiter(), body)));
            i += 1;
            continue;
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out.into_iter().collect()
}

/// Event RHS: reads only (no assignments at this level).
fn transform_event_read(
    toks: &[TokenTree],
    env: &Env,
    value_binding: Option<&str>,
    _level: Level,
) -> TokenStream {
    let mut out: Vec<TokenTree> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if let TokenTree::Ident(id) = &toks[i] {
            let name = id.to_string();
            if name == "_" {
                match value_binding {
                    Some(v) => {
                        out.extend(parse_ts(&format!("::elm_magic::clone_value(&{})", v)));
                        i += 1;
                        continue;
                    }
                    None => {
                        out.extend(parse_ts(
                            "::core::compile_error!(\"elm-magic: `_`는 on_change/on_enter 같은 값 이벤트에서만 쓸 수 있습니다\")",
                        ));
                        i += 1;
                        continue;
                    }
                }
            }
            if let Some((ts, next)) = special_form(env, toks, i, "_elm_a", "_elm_a") {
                out.extend(ts);
                i = next;
                continue;
            }
            if !is_field_access(toks, i) && env.states.contains(&name) {
                // struct-literal field key `name:` stays verbatim
                let is_field_key = matches!(toks.get(i + 1), Some(TokenTree::Punct(p))
                    if p.as_char() == ':' && p.spacing() == Spacing::Alone);
                if is_field_key {
                    out.push(toks[i].clone());
                    i += 1;
                    continue;
                }
                let (sub, next) = substitute_read(toks, i, env, "_elm_a");
                out.extend(sub);
                i = next;
                continue;
            }
        }
        if let TokenTree::Group(g) = &toks[i] {
            if let Some(sub) = shorthand_group(toks, i, env, "_elm_a") {
                out.extend(parse_ts(&sub));
                i += 1;
                continue;
            }
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            let body = transform_event_read(&inner, env, value_binding, Level::Expr);
            out.push(TokenTree::Group(Group::new(g.delimiter(), body)));
            i += 1;
            continue;
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out.into_iter().collect()
}

// ── text / fmt ──────────────────────────────────────────────

/// `"Count: {n}"` → Text with format! when braces present.
fn text_expr(content: &str, env: &Env) -> TokenStream {
    let (fmt, args) = split_fmt(content);
    let text = if args.is_empty() {
        format!("::std::string::String::from({:?})", content)
    } else {
        let mut parts: Vec<String> = Vec::new();
        for a in &args {
            let toks: Vec<TokenTree> = a.parse::<TokenStream>().unwrap().into_iter().collect();
            parts.push(transform_render(&toks, env).to_string());
        }
        format!("::std::format!({:?}, {})", fmt, parts.join(", "))
    };
    format!(
        "::elm_magic::Element::Text(::elm_magic::TextEl {{ text: {}, class: ::std::vec![] }})",
        text
    )
    .parse()
    .expect("elm-magic internal: bad text code")
}

/// Split `"a {x} b"` into ("a {} b", ["x"]) — `{x:?}`/`{x:.2}`의 서식도 지원.
fn split_fmt(s: &str) -> (String, Vec<String>) {
    let mut fmt = String::new();
    let mut args = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut inner = String::new();
            for d in chars.by_ref() {
                if d == '}' {
                    break;
                }
                inner.push(d);
            }
            // `expr:?` / `expr:.2` — 단일 `:`(경로 `::`가 아닌)이면 서식 지정자
            let mut spec: Option<usize> = None;
            let bytes: Vec<char> = inner.chars().collect();
            for (k, ch) in bytes.iter().enumerate() {
                if *ch == ':' && bytes.get(k + 1) != Some(&':') && bytes.get(k + 1) != Some(&'=') {
                    spec = Some(k);
                    break;
                }
            }
            match spec {
                Some(k) => {
                    let expr: String = bytes[..k].iter().collect();
                    let fmt_spec: String = bytes[k + 1..].iter().collect();
                    fmt.push('{');
                    fmt.push(':');
                    fmt.push_str(&fmt_spec);
                    fmt.push('}');
                    if !expr.trim().is_empty() {
                        args.push(expr);
                    }
                }
                None => {
                    fmt.push_str("{}");
                    if !inner.is_empty() {
                        args.push(inner);
                    }
                }
            }
        } else {
            fmt.push(c);
        }
    }
    (fmt, args)
}

// ── JSX element parsing ─────────────────────────────────────

enum AttrVal {
    Lit(String),
    Expr(TokenStream),
    Flag,
}

/// Parse `<Tag attrs> children </Tag>` or `<Tag attrs />` at toks[i] == `<`.
fn parse_element(toks: &[TokenTree], start: usize, env: &Env) -> (TokenStream, usize) {
    let tag = match &toks[start + 1] {
        TokenTree::Ident(id) => id.to_string(),
        _ => panic!("elm-magic: expected tag name after `<`"),
    };
    // `<Raw>|ui: &mut T| { ... }</Raw>` — no attributes; the children are a
    // user closure captured verbatim (사양서 7.3). `<Raw>` may or may not
    // carry the closing `>` of the open tag.
    if tag == "Raw" {
        let mut children_start = start + 2;
        if matches!(toks.get(children_start), Some(TokenTree::Punct(p)) if p.as_char() == '>') {
            children_start += 1;
        }
        let mut j = children_start;
        while j < toks.len() {
            if let TokenTree::Punct(p) = &toks[j] {
                if p.as_char() == '<'
                    && matches!(toks.get(j + 1), Some(TokenTree::Punct(pp)) if pp.as_char() == '/')
                    && matches!(toks.get(j + 2), Some(TokenTree::Ident(id)) if id.to_string() == "Raw")
                    && matches!(toks.get(j + 3), Some(TokenTree::Punct(gt)) if gt.as_char() == '>')
                {
                    return (emit_raw(&toks[children_start..j]), j + 4);
                }
            }
            j += 1;
        }
        panic!("elm-magic: unclosed tag `<Raw>`");
    }
    let mut i = start + 2;
    let mut attrs: Vec<(String, AttrVal)> = Vec::new();
    let mut self_closing = false;
    loop {
        match toks.get(i) {
            Some(TokenTree::Ident(key)) => {
                let key = key.to_string();
                if matches!(toks.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '=') {
                    match toks.get(i + 2) {
                        Some(TokenTree::Literal(l)) => {
                            let s = l.to_string();
                            attrs.push((key, AttrVal::Lit(s[1..s.len() - 1].to_string())));
                            i += 3;
                        }
                        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                            let is_event = key.starts_with("on_");
                            // `{_}`는 *값* 이벤트(on_change/on_enter)와
                            // 컴포넌트 콜백 prop(`on_select: fn(Id)`)에서 전달값을 뜻한다.
                            let value_binding = if matches!(key.as_str(), "on_change" | "on_enter")
                                || (!is_builtin_tag(&tag) && is_event)
                            {
                                Some("_elm_v")
                            } else {
                                None
                            };
                            let expr = if is_event {
                                transform_event(&inner, env, value_binding, Level::Stmt)
                            } else {
                                transform_render(&inner, env)
                            };
                            attrs.push((key, AttrVal::Expr(expr)));
                            i += 3;
                        }
                        _ => panic!("elm-magic: bad attribute value for `{}`", key),
                    }
                } else {
                    attrs.push((key, AttrVal::Flag));
                    i += 1;
                }
            }
            Some(TokenTree::Punct(p)) if p.as_char() == '/' => {
                self_closing = true;
                i += 2; // `/` `>`
                break;
            }
            Some(TokenTree::Punct(p)) if p.as_char() == '>' => {
                i += 1;
                break;
            }
            other => panic!("elm-magic: unexpected token in tag: {:?}", other),
        }
    }

    if self_closing {
        return (emit_element(&tag, &attrs, None, env), i);
    }

    // scan children until `</ Tag >`, tracking nested same-tag opens
    let children_start = i;
    let mut depth = 0usize;
    let mut j = i;
    while j < toks.len() {
        if let TokenTree::Punct(p) = &toks[j] {
            if p.as_char() == '<' {
                if matches!(toks.get(j + 1), Some(TokenTree::Punct(pp)) if pp.as_char() == '/') {
                    if let (Some(TokenTree::Ident(id)), Some(TokenTree::Punct(gt))) =
                        (toks.get(j + 2), toks.get(j + 3))
                    {
                        if id.to_string() == tag && gt.as_char() == '>' {
                            if depth == 0 {
                                let children: Vec<TokenTree> = toks[children_start..j].to_vec();
                                return (emit_element(&tag, &attrs, Some(&children), env), j + 4);
                            }
                            depth -= 1;
                        }
                    }
                    j += 4;
                    continue;
                }
                if matches!(toks.get(j + 1), Some(TokenTree::Ident(id2)) if id2.to_string() == tag)
                {
                    depth += 1;
                }
            }
        }
        j += 1;
    }
    panic!("elm-magic: unclosed tag `<{}>`", tag);
}

// ── element code emission ───────────────────────────────────
//
// All emission builds a balanced String and parses it once, so that
// partial fragments never have to lex on their own.

fn attr_expr(attrs: &[(String, AttrVal)], key: &str) -> Option<String> {
    attrs.iter().find_map(|(k, v)| {
        if k == key {
            Some(match v {
                AttrVal::Lit(s) => format!("{:?}", s),
                AttrVal::Expr(e) => e.to_string(),
                AttrVal::Flag => "true".to_string(),
            })
        } else {
            None
        }
    })
}

/// String 필드용 속성: 리터럴도 표현식도 `String`으로 맞춘다.
fn attr_string(attrs: &[(String, AttrVal)], key: &str, default: &str) -> String {
    match attrs.iter().find(|(k, _)| k == key) {
        Some((_, AttrVal::Lit(s))) => format!("::std::string::String::from({:?})", s),
        Some((_, AttrVal::Expr(e))) => e.to_string(),
        Some((_, AttrVal::Flag)) => format!("::std::string::String::from({:?})", key),
        None => default.to_string(),
    }
}

/// `class` 속성 → `Vec<String>` 표현식.
///
/// - `class="a b"` → 컴파일타임에 공백으로 나눠 **여러 클래스**로 만든다
///   (예전에는 문자열 하나로 들어가 어떤 셀렉터와도 안 맞았다).
/// - `class={expr}` → `IntoClasses`가 문자열/목록을 받는다 (사양서 6.2).
fn class_tokens(attrs: &[(String, AttrVal)]) -> String {
    let value = attrs
        .iter()
        .find_map(|(k, v)| if k == "class" { Some(v) } else { None });
    match value {
        Some(AttrVal::Lit(s)) => {
            let parts: Vec<String> = s
                .split_whitespace()
                .map(|p| format!("::std::string::String::from({:?})", p))
                .collect();
            format!("::std::vec![{}]", parts.join(", "))
        }
        Some(AttrVal::Expr(e)) => {
            format!("::elm_magic::style::IntoClasses::into_classes({})", e)
        }
        // `class`가 없거나 플래그만 있으면 빈 목록
        _ => "::std::vec![]".to_string(),
    }
}

fn event_closure(attrs: &[(String, AttrVal)], key: &str, value_ty: Option<&str>) -> String {
    let body = attrs.iter().find_map(|(k, v)| match (k, v) {
        (k, AttrVal::Expr(e)) if k == key => Some(e.to_string()),
        _ => None,
    });
    let body = match body {
        Some(b) => b,
        // 이벤트 속성이 없으면 핸들러도 만들지 않는다
        None => return "::std::option::Option::None".to_string(),
    };
    let sig = match value_ty {
        Some(t) => format!(
            "::std::option::Option::Some(::std::rc::Rc::new(move |_elm_a: &mut ::elm_magic::Arena, _elm_v: {}| {{ ",
            t
        ),
        None => "::std::option::Option::Some(::std::rc::Rc::new(move |_elm_a: &mut ::elm_magic::Arena| { ".to_string(),
    };
    format!("{}{}}}))", sig, body)
}

/// Text/Button `text` value from children pieces (literals + brace exprs).
fn text_from_children(children: &[TokenTree], env: &Env) -> String {
    let mut fmt = String::new();
    let mut args: Vec<String> = Vec::new();
    let mut k = 0;
    while k < children.len() {
        match &children[k] {
            TokenTree::Literal(l) => {
                let s = l.to_string();
                if s.starts_with('"') {
                    let content = &s[1..s.len() - 1];
                    if content.contains('{') {
                        // interpolated literal: "a {x}" contributes fmt + args
                        let (f, a) = split_fmt(content);
                        fmt.push_str(&f);
                        for x in a {
                            let toks: Vec<TokenTree> =
                                x.parse::<TokenStream>().unwrap().into_iter().collect();
                            args.push(transform_render(&toks, env).to_string());
                        }
                    } else {
                        fmt.push_str(content);
                    }
                }
                k += 1;
            }
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                fmt.push_str("{}");
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                args.push(transform_render(&inner, env).to_string());
                k += 1;
            }
            _ => panic!("elm-magic: unsupported child in text position"),
        }
    }
    if args.is_empty() {
        format!("::std::string::String::from({:?})", fmt)
    } else {
        format!("::std::format!({:?}, {})", fmt, args.join(", "))
    }
}

fn emit_element(
    tag: &str,
    attrs: &[(String, AttrVal)],
    children: Option<&[TokenTree]>,
    env: &Env,
) -> TokenStream {
    let code = match tag {
        "Text" => {
            let text = match children {
                Some(c) if !c.is_empty() => text_from_children(c, env),
                _ => attr_string(attrs, "text", "::std::string::String::new()"),
            };
            format!(
                "::elm_magic::Element::Text(::elm_magic::TextEl {{ text: {}, class: {} }})",
                text,
                class_tokens(attrs)
            )
        }
        "Col" | "Row" => {
            let children_ts = transform_children(children.unwrap_or(&[]), env, ',').to_string();
            format!(
                "::elm_magic::Element::{}(::elm_magic::{}El {{ class: {}, children: {}, on_click: {} }})",
                tag,
                tag,
                class_tokens(attrs),
                children_ts,
                event_closure(attrs, "on_click", None)
            )
        }
        "Button" => {
            let text = match children {
                Some(c) if !c.is_empty() => text_from_children(c, env),
                _ => attr_string(attrs, "text", "::std::string::String::new()"),
            };
            format!(
                "::elm_magic::Element::Button(::elm_magic::ButtonEl {{ text: {}, class: {}, disabled: {}, on_click: {} }})",
                text,
                class_tokens(attrs),
                attr_expr(attrs, "disabled").unwrap_or_else(|| "false".to_string()),
                event_closure(attrs, "on_click", None),
            )
        }
        "Input" => {
            format!(
                "::elm_magic::Element::Input(::elm_magic::InputEl {{ value: {}, class: {}, on_change: {}, on_enter: {} }})",
                attr_string(attrs, "value", "::std::string::String::new()"),
                class_tokens(attrs),
                event_closure(attrs, "on_change", Some("::std::string::String")),
                event_closure(attrs, "on_enter", Some("::std::string::String")),
            )
        }
        "TextArea" => {
            format!(
                "::elm_magic::Element::TextArea(::elm_magic::TextAreaEl {{ value: {}, class: {}, on_change: {}, on_enter: {} }})",
                attr_string(attrs, "value", "::std::string::String::new()"),
                class_tokens(attrs),
                event_closure(attrs, "on_change", Some("::std::string::String")),
                event_closure(attrs, "on_enter", Some("::std::string::String")),
            )
        }
        "Check" => {
            let label = match children {
                Some(c) if !c.is_empty() => text_from_children(c, env),
                _ => attr_string(attrs, "label", "::std::string::String::new()"),
            };
            format!(
                "::elm_magic::Element::Check(::elm_magic::CheckEl {{ checked: {}, label: {}, class: {}, on_change: {} }})",
                attr_expr(attrs, "checked").unwrap_or_else(|| "false".to_string()),
                label,
                class_tokens(attrs),
                event_closure(attrs, "on_change", Some("bool")),
            )
        }
        "Strong" => {
            let text = match children {
                Some(c) if !c.is_empty() => text_from_children(c, env),
                _ => attr_string(attrs, "text", "::std::string::String::new()"),
            };
            format!(
                "::elm_magic::Element::Strong(::elm_magic::StrongEl {{ text: {}, class: {} }})",
                text,
                class_tokens(attrs)
            )
        }
        "Banner" => {
            let text = match children {
                Some(c) if !c.is_empty() => text_from_children(c, env),
                _ => attr_string(attrs, "text", "::std::string::String::new()"),
            };
            format!(
                "::elm_magic::Element::Banner(::elm_magic::BannerEl {{ kind: {}, text: {}, class: {} }})",
                attr_string(attrs, "kind", "::std::string::String::from(\"info\")"),
                text,
                class_tokens(attrs)
            )
        }
        "Tab" => {
            let text = match children {
                Some(c) if !c.is_empty() => text_from_children(c, env),
                _ => attr_string(attrs, "text", "::std::string::String::new()"),
            };
            format!(
                "::elm_magic::Element::Tab(::elm_magic::TabEl {{ text: {}, active: {}, class: {}, on_click: {} }})",
                text,
                attr_expr(attrs, "active").unwrap_or_else(|| "false".to_string()),
                class_tokens(attrs),
                event_closure(attrs, "on_click", None),
            )
        }
        "Th" => {
            let text = match children {
                Some(c) if !c.is_empty() => text_from_children(c, env),
                _ => attr_string(attrs, "text", "::std::string::String::new()"),
            };
            format!(
                "::elm_magic::Element::Th(::elm_magic::ThEl {{ text: {}, class: {}, on_click: {} }})",
                text,
                class_tokens(attrs),
                event_closure(attrs, "on_click", None),
            )
        }
        "Td" => {
            let text = match children {
                Some(c) if !c.is_empty() => text_from_children(c, env),
                _ => attr_string(attrs, "text", "::std::string::String::new()"),
            };
            format!(
                "::elm_magic::Element::Td(::elm_magic::TdEl {{ text: {}, class: {} }})",
                text,
                class_tokens(attrs)
            )
        }
        "Spinner" => format!(
            "::elm_magic::Element::Spinner(::elm_magic::SpinnerEl {{ class: {} }})",
            class_tokens(attrs)
        ),
        "Divider" => format!(
            "::elm_magic::Element::Divider(::elm_magic::DividerEl {{ class: {} }})",
            class_tokens(attrs)
        ),
        "Progress" => format!(
            "::elm_magic::Element::Progress(::elm_magic::ProgressEl {{ value: {}, class: {} }})",
            attr_expr(attrs, "value").unwrap_or_else(|| "0.0".to_string()),
            class_tokens(attrs)
        ),
        "Modal" => {
            let children_ts = transform_children(children.unwrap_or(&[]), env, ',').to_string();
            format!(
                "::elm_magic::Element::Modal(::elm_magic::ModalEl {{ title: {}, class: {}, on_close: {}, children: {} }})",
                attr_string(attrs, "title", "::std::string::String::new()"),
                class_tokens(attrs),
                event_closure(attrs, "on_close", None),
                children_ts
            )
        }
        "Raw" => unreachable!("`<Raw>` is handled by emit_raw in parse_element"),
        other => return emit_component(other, attrs, children, env),
    };
    // `<... key={…}>` — 자식 컴포넌트의 슬롯 경로에 키를 넣는다 (사양서 9.5).
    let code = match key_attr(attrs) {
        Some(k) if is_builtin_tag(tag) => format!(
            "{{ __elm_ctx.enter_key(::elm_magic::key_of(&({}))); let __elm_keyed = {}; __elm_ctx.exit_key(); __elm_keyed }}",
            k, code
        ),
        _ => code,
    };
    code.parse().expect("elm-magic internal: bad element code")
}

/// `<Raw>|ui: &mut T| { ... }</Raw>` → `Element::Raw` wrapping the user's
/// closure verbatim, downcast by `raw::call_raw` at invocation time.
fn emit_raw(children: &[TokenTree]) -> TokenStream {
    if children.is_empty() {
        panic!("elm-magic: <Raw> expects a closure: <Raw>|ui: &mut T| {{ ... }}</Raw>");
    }
    let template = parse_ts(
        "::elm_magic::Element::Raw(::elm_magic::RawEl { class: ::std::vec![], widget: \
         ::std::rc::Rc::new(move |__elm_payload: &mut dyn ::std::any::Any| \
         ::elm_magic::raw::call_raw(__elm_payload, __elm_user_closure)) })",
    );
    let result = substitute_ident(
        template,
        "__elm_user_closure",
        children.iter().cloned().collect(),
    );
    result
}

/// Replace every occurrence of an ident with the given token stream.
fn substitute_ident(ts: TokenStream, name: &str, replacement: TokenStream) -> TokenStream {
    ts.into_iter()
        .flat_map(|t| match t {
            TokenTree::Ident(ref id) if id.to_string() == name => replacement.clone(),
            TokenTree::Group(g) => {
                let inner = substitute_ident(g.stream(), name, replacement.clone());
                TokenStream::from(TokenTree::Group(Group::new(g.delimiter(), inner)))
            }
            other => TokenStream::from(other),
        })
        .collect()
}

/// 내장 태그인가? (아니면 사용자 컴포넌트)
fn is_builtin_tag(tag: &str) -> bool {
    matches!(
        tag,
        "Text"
            | "Strong"
            | "Col"
            | "Row"
            | "Button"
            | "Input"
            | "TextArea"
            | "Check"
            | "Tab"
            | "Th"
            | "Td"
            | "Banner"
            | "Spinner"
            | "Divider"
            | "Progress"
            | "Modal"
            | "Raw"
    )
}

/// 사용자 컴포넌트 렌더 (사양서 3.1):
/// `Some(prop)`으로 감싸고(모든 prop은 `Option<T>`), `on_*`는 콜백 prop으로,
/// `key`는 인스턴스 경로로, 자식은 `children` prop으로 넘긴다.
fn emit_component(
    tag: &str,
    attrs: &[(String, AttrVal)],
    children: Option<&[TokenTree]>,
    env: &Env,
) -> TokenStream {
    if !tag.starts_with(char::is_uppercase) {
        panic!(
            "elm-magic: unknown tag `<{}>` (builtins: Col, Row, Text, Strong, Button, Input, \
             TextArea, Check, Tab, Th, Td, Banner, Spinner, Divider, Progress, Modal, Raw)",
            tag
        );
    }
    let mut fields = String::new();
    for (k, v) in attrs {
        if k == "key" {
            continue;
        }
        match v {
            AttrVal::Flag => continue,
            AttrVal::Expr(e) if k.starts_with("on_") => {
                // 콜백 prop — `on_select={selected = Some(_)}` (사양서 3.1)
                fields.push_str(&format!(
                    "{}: ::core::option::Option::Some(::elm_magic::Callback::new(move |_elm_a: &mut ::elm_magic::Arena, _elm_v| {{ {} }})), ",
                    k, e
                ));
            }
            AttrVal::Lit(s) => fields.push_str(&format!(
                "{}: ::core::option::Option::Some(::std::string::String::from({:?})), ",
                k, s
            )),
            AttrVal::Expr(e) => {
                fields.push_str(&format!("{}: ::core::option::Option::Some({}), ", k, e))
            }
        }
    }
    if let Some(c) = children {
        if !c.is_empty() {
            let children_ts = transform_children(c, env, ',').to_string();
            fields.push_str(&format!(
                "children: ::core::option::Option::Some({}), ",
                children_ts
            ));
        }
    }
    // 순서 중요: 키 → 경로 세그먼트 → props → 인스턴스 렌더.
    // (props를 먼저 만들면 그 안의 자식 인스턴스들이 형제 순번을 먼저 가져가
    //  부모 경로가 흔들린다. `key={i.id}`가 `i`를 borrow하는 것과도 순서가 맞아야 한다.)
    let key = key_attr(attrs)
        .map(|k| {
            format!(
                "::core::option::Option::Some(::elm_magic::key_of(&({})))",
                k
            )
        })
        .unwrap_or_else(|| "::core::option::Option::None".to_string());
    let code = format!(
        "{{ let __elm_key = {k};           let __elm_seg = __elm_ctx.instance_segment(__elm_key, {t:?});           let __elm_p = {t}Props {{ {f}..::core::default::Default::default() }};           let __elm_e = __elm_ctx.with_instance(__elm_seg, |__elm_ctx| {t}::render(__elm_ctx, &__elm_p));           __elm_e }}",
        k = key,
        t = tag,
        f = fields
    );
    code.parse()
        .expect("elm-magic internal: bad component code")
}

/// 분기 본문(블록) → 값 식.
///
/// 분기 본문은 **표현식**이다 — 요소(`<Text>..`), 이터레이터(`items.map(..)`),
/// 중첩 조건부 모두 `Vec<Element>`로 통일한다. 문자열 리터럴만 있는 분기는
/// Text 요소로 바꾼다(`{if ok { "OK" } else { "NG" }}`).
fn element_body(g: &Group, env: &Env) -> String {
    let inner: Vec<TokenTree> = g.stream().into_iter().collect();
    if inner.len() == 1 {
        if let TokenTree::Literal(l) = &inner[0] {
            let s = l.to_string();
            if s.starts_with('"') {
                return text_expr(&s[1..s.len() - 1], env).to_string();
            }
        }
    }
    transform_element_expr(&inner, env).to_string()
}

/// `if` / `else if` / `else` 사슬을 분기마다 `Vec<Element>`로 통일한다 (사양서 3.4).
fn unify_if(toks: &[TokenTree], env: &Env) -> Option<String> {
    if !matches!(toks.first(), Some(TokenTree::Ident(id)) if id.to_string() == "if") {
        return None;
    }
    let mut out = String::new();
    let mut i = 0;
    while i < toks.len() {
        let word = match &toks[i] {
            TokenTree::Ident(id) => id.to_string(),
            _ => return None,
        };
        match word.as_str() {
            "if" => {
                i += 1;
                let mut cond: Vec<TokenTree> = Vec::new();
                while i < toks.len() {
                    if matches!(&toks[i], TokenTree::Group(g) if g.delimiter() == Delimiter::Brace)
                    {
                        break;
                    }
                    cond.push(toks[i].clone());
                    i += 1;
                }
                let body = match toks.get(i) {
                    Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => g,
                    _ => return None,
                };
                i += 1;
                if !out.is_empty() {
                    out.push_str("else ");
                }
                out.push_str(&format!(
                    "if {} {{ ::elm_magic::IntoElements::into_elements({}) }} ",
                    transform_render(&cond, env),
                    element_body(body, env)
                ));
            }
            "else" => {
                i += 1;
                match toks.get(i) {
                    Some(TokenTree::Ident(id)) if id.to_string() == "if" => continue,
                    Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                        i += 1;
                        out.push_str(&format!(
                            "else {{ ::elm_magic::IntoElements::into_elements({}) }}",
                            element_body(g, env)
                        ));
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
        if i >= toks.len() {
            break;
        }
        if !matches!(&toks[i], TokenTree::Ident(id) if id.to_string() == "else") {
            return None;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// `match` arm들을 `Vec<Element>`로 통일한다 (사양서 3.4).
fn unify_match(toks: &[TokenTree], env: &Env) -> Option<String> {
    if !matches!(toks.first(), Some(TokenTree::Ident(id)) if id.to_string() == "match") {
        return None;
    }
    let mut i = 1;
    let mut scrutinee: Vec<TokenTree> = Vec::new();
    while i < toks.len() {
        if matches!(&toks[i], TokenTree::Group(g) if g.delimiter() == Delimiter::Brace) {
            break;
        }
        scrutinee.push(toks[i].clone());
        i += 1;
    }
    let arms = match toks.get(i) {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => g,
        _ => return None,
    };
    if i + 1 != toks.len() {
        return None;
    }
    let arm_toks: Vec<TokenTree> = arms.stream().into_iter().collect();
    let arms_src = unify_arms(&arm_toks, env)?;
    Some(format!(
        "match {} {{ {} }}",
        transform_render(&scrutinee, env),
        arms_src
    ))
}

/// arm 목록: `pat => body` → `pat => into_elements(body)`.
fn unify_arms(toks: &[TokenTree], env: &Env) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let mut pat: Vec<TokenTree> = Vec::new();
        while i < toks.len() {
            if matches!(
                (&toks[i], toks.get(i + 1)),
                (TokenTree::Punct(a), Some(TokenTree::Punct(b)))
                    if a.as_char() == '=' && b.as_char() == '>'
            ) {
                break;
            }
            pat.push(toks[i].clone());
            i += 1;
        }
        if pat.is_empty() || i + 1 >= toks.len() {
            return None;
        }
        i += 2; // `=>`
        let body_src = match toks.get(i) {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                let src = format!("{{ {} }}", element_body(g, env));
                i += 1;
                src
            }
            _ => {
                let mut b: Vec<TokenTree> = Vec::new();
                while i < toks.len() {
                    if matches!(&toks[i], TokenTree::Punct(p) if p.as_char() == ',') {
                        break;
                    }
                    b.push(toks[i].clone());
                    i += 1;
                }
                transform_render(&b, env).to_string()
            }
        };
        out.push(format!(
            "{} => ::elm_magic::IntoElements::into_elements({})",
            TokenStream::from_iter(pat).to_string(),
            body_src
        ));
        if matches!(toks.get(i), Some(TokenTree::Punct(p)) if p.as_char() == ',') {
            i += 1;
        }
    }
    Some(out.join(", "))
}

/// children 위치의 중괄호 식 — `if`/`match`는 분기 타입을 통일해 감싼다.
fn transform_element_expr(inner: &[TokenTree], env: &Env) -> TokenStream {
    if let Some(ts) = unify_if(inner, env) {
        return parse_ts(&format!("::elm_magic::IntoElements::into_elements({})", ts));
    }
    if let Some(ts) = unify_match(inner, env) {
        return parse_ts(&format!("::elm_magic::IntoElements::into_elements({})", ts));
    }
    transform_render(inner, env)
}
