use proc_macro::{Delimiter, Group, Punct, Spacing, Span, TokenStream, TokenTree};
use std::collections::HashSet;

pub struct Env<'a> {
    pub states: &'a HashSet<String>,
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
    s.parse().unwrap_or_else(|e| {
        panic!("elm-magic internal: bad generated code {:?}: {:?}", s, e)
    })
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
    // `items.map(...)` sugar → `(state).iter().map(...)`
    if let (Some(TokenTree::Punct(dot)), Some(TokenTree::Ident(m))) =
        (toks.get(i + 1), toks.get(i + 2))
    {
        if dot.as_char() == '.' && m.to_string() == "map" {
            let out = parse_ts(&format!(
                "({}.get({}).clone()).iter().map",
                state_var, arena
            ));
            return (out, i + 3);
        }
    }
    let out = parse_ts(&format!("({}.get({}).clone())", state_var, arena));
    (out, i + 1)
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
                let expr = transform_render(&inner, env);
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
fn transform_tokens(toks: &[TokenTree], env: &Env, mode: Mode) -> TokenStream {
    match mode {
        Mode::EventWrite => transform_event(toks, env, None),
        Mode::EventRead => transform_event_read(toks, env, None),
        Mode::Render => transform_render(toks, env),
    }
}

fn transform_render(toks: &[TokenTree], env: &Env) -> TokenStream {
    let mut out: Vec<TokenTree> = Vec::new();
    let mut i = 0;
    let mut prev_dot = false;
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
                prev_dot = false;
                continue;
            }
        }
        // interpolated string literal in element position: "x {y}" → Text
        if let TokenTree::Literal(l) = &toks[i] {
            let s = l.to_string();
            if s.starts_with('"') && s.contains('{') {
                out.extend(text_expr(&s[1..s.len() - 1], env));
                i += 1;
                prev_dot = false;
                continue;
            }
        }
        if let TokenTree::Ident(id) = &toks[i] {
            let name = id.to_string();
            // field key (`name:`) and field access (`.name`) stay verbatim
            let is_field_key = matches!(toks.get(i + 1), Some(TokenTree::Punct(p))
                if p.as_char() == ':' && p.spacing() == Spacing::Alone);
            if !prev_dot && !is_field_key && env.states.contains(&name) {
                let (sub, next) = substitute_read(toks, i, env, "&__elm_ctx.arena");
                out.extend(sub);
                i = next;
                prev_dot = false;
                continue;
            }
        }
        if let TokenTree::Punct(p) = &toks[i] {
            if p.as_char() == '<' && !prev_colon {
                if let Some(TokenTree::Ident(_)) = toks.get(i + 1) {
                    let (el, next) = parse_element(toks, i, env);
                    out.extend(el);
                    i = next;
                    prev_dot = false;
                    prev_colon = false;
                    continue;
                }
            }
            prev_dot = p.as_char() == '.';
            prev_colon = p.as_char() == ':';
        }
        if let TokenTree::Group(g) = &toks[i] {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            let body = transform_render(&inner, env);
            out.push(TokenTree::Group(Group::new(g.delimiter(), body)));
            i += 1;
            prev_dot = false;
            continue;
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out.into_iter().collect()
}

// ── events ──────────────────────────────────────────────────

/// Event handler top level: `n += 1`, `text = e`, `items.push(x)` → slot ops.
fn transform_event(toks: &[TokenTree], env: &Env, value_binding: Option<&str>) -> TokenStream {
    let mut out: Vec<TokenTree> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if let TokenTree::Ident(id) = &toks[i] {
            let name = id.to_string();
            if name == "_" {
                if let Some(v) = value_binding {
                    out.extend(parse_ts(&format!("({}.clone())", v)));
                    i += 1;
                    continue;
                }
            }
            if env.states.contains(&name) {
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
                    if plain_assign || op_assign {
                        let rhs_start = if plain_assign { i + 2 } else { i + 3 };
                        let rhs_end = toks[rhs_start..]
                            .iter()
                            .position(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ';'))
                            .map(|p| rhs_start + p)
                            .unwrap_or(toks.len());
                        let rhs: Vec<TokenTree> = toks[rhs_start..rhs_end].to_vec();
                        let rhs_ts = transform_event_read(&rhs, env, value_binding).to_string();
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
                        if i < toks.len() {
                            out.push(punct(';'));
                            i += 1;
                        }
                        continue;
                    }
                    // method call: items.push(x)
                    if c1 == '.' {
                        if let (Some(TokenTree::Ident(m)), Some(TokenTree::Group(g))) =
                            (toks.get(i + 2), toks.get(i + 3))
                        {
                            if g.delimiter() == Delimiter::Parenthesis {
                                let args: Vec<TokenTree> = g.stream().into_iter().collect();
                                let args_ts = transform_event_read(&args, env, value_binding).to_string();
                                // precompute args before taking the mutable borrow
                                let n_args = if args.is_empty() {
                                    0
                                } else {
                                    count_top_level_commas(&args)
                                };
                                let mut tuple = String::new();
                                let mut indices = String::new();
                                if n_args > 0 {
                                    tuple = format!("({},)", args_ts);
                                    for k in 0..n_args {
                                        if k > 0 {
                                            indices.push_str(", ");
                                        }
                                        indices.push_str(&format!("__elm_args.{}", k));
                                    }
                                }
                                let emitted = format!(
                                    "{{ let __elm_args = {}; {}.mutate(_elm_a, |__v| __v.{}({})); }}",
                                    tuple, state_var, m.to_string(), indices
                                );
                                out.extend(parse_ts(&emitted));
                                i += 4;
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
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            if g.delimiter() == Delimiter::Brace && inner.len() == 1 {
                if let TokenTree::Ident(id) = &inner[0] {
                    let name = id.to_string();
                    if env.states.contains(&name) {
                        // struct-literal shorthand `Todo { text }`
                        out.extend(parse_ts(&format!(
                            "{}: (__elm_state_{}.get(_elm_a).clone())",
                            name, name
                        )));
                        i += 1;
                        continue;
                    }
                }
            }
            let body = transform_event(&inner, env, value_binding);
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
fn transform_event_read(toks: &[TokenTree], env: &Env, value_binding: Option<&str>) -> TokenStream {
    let mut out: Vec<TokenTree> = Vec::new();
    let mut i = 0;
    let mut prev_dot = false;
    while i < toks.len() {
        if let TokenTree::Ident(id) = &toks[i] {
            let name = id.to_string();
            if name == "_" {
                if let Some(v) = value_binding {
                    out.extend(parse_ts(&format!("({}.clone())", v)));
                    i += 1;
                    prev_dot = false;
                    continue;
                }
            }
            if !prev_dot && env.states.contains(&name) {
                // struct-literal field key `name:` stays verbatim
                let is_field_key = matches!(toks.get(i + 1), Some(TokenTree::Punct(p))
                    if p.as_char() == ':' && p.spacing() == Spacing::Alone);
                if is_field_key {
                    out.push(toks[i].clone());
                    i += 1;
                    prev_dot = false;
                    continue;
                }
                let (sub, next) = substitute_read(toks, i, env, "_elm_a");
                out.extend(sub);
                i = next;
                prev_dot = false;
                continue;
            }
        }
        if let TokenTree::Punct(p) = &toks[i] {
            prev_dot = p.as_char() == '.';
        }
        if let TokenTree::Group(g) = &toks[i] {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            if g.delimiter() == Delimiter::Brace && inner.len() == 1 {
                if let TokenTree::Ident(id) = &inner[0] {
                    let name = id.to_string();
                    if env.states.contains(&name) {
                        out.extend(parse_ts(&format!(
                            "{}: (__elm_state_{}.get(_elm_a).clone())",
                            name, name
                        )));
                        i += 1;
                        continue;
                    }
                }
            }
            let body = transform_event_read(&inner, env, value_binding);
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
            let toks: Vec<TokenTree> =
                a.parse::<TokenStream>().unwrap().into_iter().collect();
            parts.push(transform_render(&toks, env).to_string());
        }
        format!("::std::format!({:?}, {})", fmt, parts.join(", "))
    };
    format!(
        "::elm_magic::Element::Text {{ text: {}, class: ::std::vec![] }}",
        text
    )
    .parse()
    .expect("elm-magic internal: bad text code")
}

/// Split `"a {x} b"` into ("a {} b", ["x"]).
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
            fmt.push_str("{}");
            if !inner.is_empty() {
                args.push(inner);
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
                            let mode = if key.starts_with("on_") {
                                Mode::EventWrite
                            } else {
                                Mode::Render
                            };
                            let expr = match mode {
                                // `{_}` means "the incoming value" in value events
                                Mode::EventWrite => {
                                    transform_event(&inner, env, Some("_elm_v"))
                                }
                                _ => transform_tokens(&inner, env, mode),
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
                                return (
                                    emit_element(&tag, &attrs, Some(&children), env),
                                    j + 4,
                                );
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

fn class_tokens(attrs: &[(String, AttrVal)]) -> String {
    let inner = attrs.iter().find_map(|(k, v)| {
        if k == "class" {
            Some(match v {
                AttrVal::Lit(s) => format!("::std::convert::Into::into({:?})", s),
                AttrVal::Expr(e) => format!("::std::convert::Into::into({})", e),
                AttrVal::Flag => "::std::convert::Into::into(\"\")".to_string(),
            })
        } else {
            None
        }
    });
    match inner {
        Some(x) => format!("::std::vec![{}]", x),
        None => "::std::vec![]".to_string(),
    }
}

fn event_closure(
    attrs: &[(String, AttrVal)],
    key: &str,
    value_ty: Option<&str>,
) -> String {
    let body = attrs.iter().find_map(|(k, v)| match (k, v) {
        (k, AttrVal::Expr(e)) if k == key => Some(e.to_string()),
        _ => None,
    });
    let sig = match value_ty {
        Some(t) => format!(
            "::std::option::Option::Some(::std::rc::Rc::new(move |_elm_a: &mut ::elm_magic::Arena, _elm_v: {}| {{ ",
            t
        ),
        None => "::std::option::Option::Some(::std::rc::Rc::new(move |_elm_a: &mut ::elm_magic::Arena| { ".to_string(),
    };
    format!("{}{}}}))", sig, body.unwrap_or_default())
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
                    fmt.push_str(&s[1..s.len() - 1]);
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
                _ => attr_expr(attrs, "text")
                    .unwrap_or_else(|| "::std::string::String::new()".to_string()),
            };
            format!(
                "::elm_magic::Element::Text {{ text: {}, class: {} }}",
                text,
                class_tokens(attrs)
            )
        }
        "Col" | "Row" => {
            let children_ts = transform_children(children.unwrap_or(&[]), env, ',').to_string();
            format!(
                "::elm_magic::Element::{} {{ class: {}, children: {} }}",
                tag,
                class_tokens(attrs),
                children_ts
            )
        }
        "Button" => {
            let text = match children {
                Some(c) if !c.is_empty() => text_from_children(c, env),
                _ => attr_expr(attrs, "text")
                    .unwrap_or_else(|| "::std::string::String::new()".to_string()),
            };
            format!(
                "::elm_magic::Element::Button {{ text: {}, class: {}, disabled: {}, on_click: {} }}",
                text,
                class_tokens(attrs),
                attr_expr(attrs, "disabled").unwrap_or_else(|| "false".to_string()),
                event_closure(attrs, "on_click", None),
            )
        }
        "Input" => {
            format!(
                "::elm_magic::Element::Input {{ value: {}, class: {}, on_change: {}, on_enter: {} }}",
                attr_expr(attrs, "value")
                    .unwrap_or_else(|| "::std::string::String::new()".to_string()),
                class_tokens(attrs),
                event_closure(attrs, "on_change", Some("::std::string::String")),
                event_closure(attrs, "on_enter", Some("::std::string::String")),
            )
        }
        other => return emit_component(other, attrs, children, env),
    };
    code.parse().expect("elm-magic internal: bad element code")
}

fn emit_component(
    tag: &str,
    attrs: &[(String, AttrVal)],
    children: Option<&[TokenTree]>,
    _env: &Env,
) -> TokenStream {
    if !tag.starts_with(char::is_uppercase) {
        panic!(
            "elm-magic: unknown tag `<{}>` (builtins: Col, Row, Text, Button, Input)",
            tag
        );
    }
    if let Some(c) = children {
        if !c.is_empty() {
            panic!("elm-magic: component `<{}>` cannot have children in v0.1", tag);
        }
    }
    let mut fields = String::new();
    for (k, v) in attrs {
        match v {
            AttrVal::Flag => continue,
            AttrVal::Lit(s) => fields.push_str(&format!(
                "{}: ::std::convert::Into::into({:?}), ",
                k, s
            )),
            AttrVal::Expr(e) => fields.push_str(&format!("{}: {}, ", k, e)),
        }
    }
    let code = format!(
        "{{ let __elm_p = {t}Props {{ {f}..::core::default::Default::default() }}; let __elm_b = __elm_ctx.base; __elm_ctx.base += {t}::SLOTS; let __elm_e = {t}::render(__elm_ctx, &__elm_p); __elm_ctx.base = __elm_b; __elm_e }}",
        t = tag,
        f = fields
    );
    code.parse().expect("elm-magic internal: bad component code")
}









