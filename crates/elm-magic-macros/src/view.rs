//! `#[view]` attribute macro implementation.

use crate::jsx;
use proc_macro::{Delimiter, Group, Spacing, TokenStream, TokenTree};
use std::cell::Cell;
use std::collections::HashSet;

/// Infer the type of an untyped parameter from its default literal.
fn infer_type(default: &str) -> Option<&'static str> {
    let d = default.trim();
    let compact = d.replace(' ', "");
    if d.starts_with('"') {
        return Some("::std::string::String");
    }
    if d == "true" || d == "false" {
        return Some("bool");
    }
    if compact == "String::new()" || compact.starts_with("String::from(") {
        return Some("::std::string::String");
    }
    if let Some(tok) = d.split_whitespace().next() {
        let t = tok.trim_end_matches(',');
        if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '-') {
            return Some("i32");
        }
        if !t.is_empty()
            && t.contains('.')
            && t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        {
            return Some("f64");
        }
    }
    None
}

/// Split top-level params on ',' respecting nesting depth.
fn split_params(toks: Vec<TokenTree>) -> Vec<Vec<TokenTree>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    let mut depth = 0i32;
    for t in toks {
        if let TokenTree::Punct(p) = &t {
            match p.as_char() {
                '<' => depth += 1,
                '>' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(std::mem::take(&mut cur));
                    continue;
                }
                _ => {}
            }
        }
        cur.push(t);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn expand(item: TokenStream) -> TokenStream {
    let toks: Vec<TokenTree> = item.into_iter().collect();
    let mut i = 0;

    // visibility
    let mut vis = String::new();
    if let Some(TokenTree::Ident(id)) = toks.get(i) {
        if id.to_string() == "pub" {
            vis = "pub ".to_string();
            i += 1;
            if let Some(TokenTree::Group(g)) = toks.get(i) {
                if g.delimiter() == Delimiter::Parenthesis {
                    i += 1;
                }
            }
        }
    }

    // optional inner attributes / doc comments: skip `#` `![...]`
    while i + 1 < toks.len() {
        if let (TokenTree::Punct(p), Some(TokenTree::Group(_))) = (&toks[i], toks.get(i + 1)) {
            if p.as_char() == '#' {
                i += 2;
                continue;
            }
        }
        break;
    }

    // `fn Name(params) { body }`
    match (toks.get(i), toks.get(i + 1), toks.get(i + 2), toks.get(i + 3)) {
        (
            Some(TokenTree::Ident(f)),
            Some(TokenTree::Ident(name)),
            Some(TokenTree::Group(params)),
            Some(TokenTree::Group(body)),
        ) if f.to_string() == "fn"
            && params.delimiter() == Delimiter::Parenthesis
            && body.delimiter() == Delimiter::Brace =>
        {
            let fn_name = name.to_string();
            expand_fn(&fn_name, &vis, params.clone(), body.clone())
        }
        _ => panic!("elm-magic: view! expects `fn Name(params) {{ ... }}`"),
    }
}

fn expand_fn(fn_name: &str, vis: &str, params: Group, body: Group) -> TokenStream {
    let param_toks: Vec<TokenTree> = params.stream().into_iter().collect();
    let param_groups = split_params(param_toks);

    let mut props_fields = String::new();
    let mut default_fields: Vec<(String, Vec<TokenTree>)> = Vec::new();
    let mut slot_inits = String::new();
    let mut state_names: HashSet<String> = HashSet::new();
    let mut slot_idx = 0usize;

    for (n, p) in param_groups.iter().enumerate() {
        let pname = match p.first() {
            Some(TokenTree::Ident(id)) => id.to_string(),
            _ => panic!("elm-magic: bad parameter #{} in `{}`", n, fn_name),
        };
        // `name` | `name: Type` | `name = default` | `name: Type = default`
        let mut ty: Option<String> = None;
        let mut default: Option<String> = None;
        let mut default_toks: Option<Vec<TokenTree>> = None;
        if p.len() > 1 {
            let mut k = 1;
            if let Some(TokenTree::Punct(colon)) = p.get(k) {
                if colon.as_char() == ':' {
                    k += 1;
                    let mut type_toks = Vec::new();
                    while k < p.len() {
                        if let TokenTree::Punct(eq) = &p[k] {
                            if eq.as_char() == '=' {
                                break;
                            }
                        }
                        type_toks.push(p[k].clone());
                        k += 1;
                    }
                    ty = Some(
                        type_toks
                            .iter()
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                }
            }
            if k < p.len() {
                if let TokenTree::Punct(eq) = &p[k] {
                    if eq.as_char() == '=' {
                        default = Some(
                            p[k + 1..]
                                .iter()
                                .map(|t| t.to_string())
                                .collect::<Vec<_>>()
                                .join(" "),
                        );
                        default_toks = Some(p[k + 1..].to_vec());
                    }
                }
            }
        }
        let default = default.unwrap_or_else(|| {
            panic!(
                "elm-magic: parameter `{}` of `{}` must have a default value",
                pname, fn_name
            )
        });
        let ty = ty
            .or_else(|| infer_type(&default).map(|s| s.to_string()))
            .unwrap_or_else(|| {
                panic!(
                    "elm-magic: cannot infer type of parameter `{}` in `{}` — annotate it, e.g. `{}: Vec<Todo> = vec![]`",
                    pname, fn_name, pname
                )
            });
        props_fields.push_str(&format!("    pub {}: {},\n", pname, ty));
        // keep the raw default tokens so the generated `Default` impl uses
        // the user-provided default expression verbatim
        default_fields.push((pname.clone(), default_toks.unwrap()));
        slot_inits.push_str(&format!(
            "    let __elm_state_{p} = __elm_ctx.slot({i}usize, || __elm_props.{p}.clone());\n",
            p = pname,
            i = slot_idx
        ));
        state_names.insert(pname);
        slot_idx += 1;
    }

    let slot_cell = Cell::new(slot_idx);
    let env = jsx::Env {
        states: &state_names,
        slots: &slot_cell,
    };
    let body_toks: Vec<TokenTree> = body.stream().into_iter().collect();
    let body_ts = jsx::transform_top(&body_toks, &env);
    // lifecycle constructs (on_mount/on_tick/on_change) allocate hidden slots
    let total_slots = slot_cell.get();

    let mut head = String::new();
    head.push_str(&format!(
        "{v}#[derive(::core::clone::Clone)]\n{v}struct {n}Props {{\n",
        v = vis,
        n = fn_name
    ));
    head.push_str(&props_fields);
    head.push_str("}\n");

    // Assemble the whole expansion as one string and parse it once:
    // partial fragments (unclosed braces) cannot be parsed on their own.
    let mut code = String::new();
    code.push_str(&head);
    // Manual Default so user-provided defaults (e.g. `status = String::from("idle")`)
    // become the initial prop values, not `Default::default()` of the type.
    code.push_str(&format!(
        "impl ::core::default::Default for {n}Props {{\n    fn default() -> Self {{\n        Self {{\n",
        n = fn_name
    ));
    for (pname, toks) in &default_fields {
        code.push_str(&format!(
            "            {}: {},\n",
            pname,
            render_tokens(toks)
        ));
    }
    code.push_str("        }\n    }\n}\n");
    code.push_str(&format!(
        "{v}struct {n};\nimpl ::elm_magic::Component for {n} {{\n    type Props = {n}Props;\n    const SLOTS: usize = {s};\n    fn render(__elm_ctx: &mut ::elm_magic::Ctx, __elm_props: &{n}Props) -> ::elm_magic::Element {{\n",
        v = vis,
        n = fn_name,
        s = total_slots
    ));
    code.push_str(&slot_inits);
    // 본문의 값이 `Vec<Element>`(조건부/분기 통일)일 수 있으므로 마감한다.
    code.push_str("::elm_magic::into_element({ ");
    code.push_str(&render_tokens_stream(&body_ts));
    code.push_str(" })\n");
    code.push_str("\n    }\n}\n");
    // 디버깅용 전개 덤프: `ELM_MAGIC_DUMP=1 cargo build`
    if std::env::var_os("ELM_MAGIC_DUMP").is_some() {
        eprintln!("===== elm-magic view! {} =====\n{}", fn_name, code);
    }
    code.parse()
        .unwrap_or_else(|e| panic!("elm-magic internal: bad generated code for `{}`: {:?}", fn_name, e))
}

/// Serialize tokens honoring joint punctuation spacing, so `::` stays `::`.
fn render_tokens_stream(ts: &TokenStream) -> String {
    render_tokens(&ts.clone().into_iter().collect::<Vec<_>>())
}

fn render_tokens(toks: &[TokenTree]) -> String {
    let mut s = String::new();
    for t in toks {
        match t {
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                s.push_str(open);
                s.push_str(&render_tokens(&g.stream().into_iter().collect::<Vec<_>>()));
                s.push_str(close);
                s.push(' ');
            }
            TokenTree::Punct(p) => {
                s.push(p.as_char());
                if p.spacing() == Spacing::Alone {
                    s.push(' ');
                }
            }
            other => {
                s.push_str(&other.to_string());
                s.push(' ');
            }
        }
    }
    s
}

