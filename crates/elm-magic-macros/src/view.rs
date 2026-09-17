//! `#[view]` attribute macro implementation.

use crate::jsx;
use crate::store;
use proc_macro::{Delimiter, Group, Spacing, TokenStream, TokenTree};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

/// 본문에서 쓰인 식별자를 모은다 (중첩 그룹 + 문자열 보간 `"{app.count}"` 포함).
fn collect_idents(toks: &[TokenTree], out: &mut HashSet<String>) {
    for t in toks {
        match t {
            TokenTree::Ident(id) => {
                out.insert(id.to_string());
            }
            TokenTree::Literal(l) => {
                let s = l.to_string();
                if s.contains('{') {
                    for chunk in s.split('{').skip(1) {
                        let inner = chunk.split('}').next().unwrap_or("");
                        for word in inner.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
                            if !word.is_empty() && !word.chars().next().unwrap().is_ascii_digit() {
                                out.insert(word.to_string());
                            }
                        }
                    }
                }
            }
            TokenTree::Group(g) => {
                collect_idents(&g.stream().into_iter().collect::<Vec<_>>(), out)
            }
            _ => {}
        }
    }
}

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
    let mut slot_inits = String::new();
    let mut default_inits: Vec<String> = Vec::new();
    let mut state_names: HashSet<String> = HashSet::new();
    let mut callbacks: HashSet<String> = HashSet::new();
    let mut slot_idx = 0usize;

    for (n, p) in param_groups.iter().enumerate() {
        let pname = match p.first() {
            Some(TokenTree::Ident(id)) => id.to_string(),
            _ => panic!("elm-magic: bad parameter #{} in `{}`", n, fn_name),
        };
        // `name = default` | `name: Type = default` | `name: Type` | `name: fn(T)`
        let mut ty: Option<String> = None;
        let mut is_fn = false;
        let mut default_toks: Option<Vec<TokenTree>> = None;
        let mut k = 1;
        if let Some(TokenTree::Punct(colon)) = p.get(k) {
            if colon.as_char() == ':' {
                k += 1;
                if matches!(p.get(k), Some(TokenTree::Ident(id)) if id.to_string() == "fn") {
                    is_fn = true;
                    k += 1;
                    let arg = match p.get(k) {
                        Some(TokenTree::Group(g)) => g.stream().to_string(),
                        _ => "()".to_string(),
                    };
                    k += 1;
                    ty = Some(if arg.trim().is_empty() { "()".to_string() } else { arg });
                } else {
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
        }
        if k + 1 < p.len() {
            if let TokenTree::Punct(eq) = &p[k] {
                if eq.as_char() == '=' {
                    default_toks = Some(p[k + 1..].to_vec());
                }
            }
        }

        // 콜백 prop: 슬롯이 아니라 props에 산다 (`on_select: fn(Id)`).
        if is_fn {
            let arg = ty.unwrap_or_else(|| "()".to_string());
            props_fields.push_str(&format!(
                "    pub {}: ::core::option::Option<::elm_magic::Callback<{}>>,\n",
                pname, arg
            ));
            default_inits.push(format!("{}: ::core::option::Option::None", pname));
            callbacks.insert(pname);
            continue;
        }

        let ty = match ty {
            Some(t) => t,
            None => match default_toks.as_ref().map(|t| render_tokens(t)) {
                Some(d) => infer_type(&d).map(|s| s.to_string()).unwrap_or_else(|| {
                    panic!(
                        "elm-magic: cannot infer type of parameter `{}` in `{}` — annotate it, e.g. `{}: Vec<Todo> = vec![]`",
                        pname, fn_name, pname
                    )
                }),
                None => panic!(
                    "elm-magic: parameter `{}` of `{}` needs a type annotation (e.g. `{}: Item`) or a default value",
                    pname, fn_name, pname
                ),
            },
        };
        props_fields.push_str(&format!(
            "    pub {}: ::core::option::Option<{}>,\n",
            pname, ty
        ));
        default_inits.push(format!("{}: ::core::option::Option::None", pname));
        match &default_toks {
            Some(d) => slot_inits.push_str(&format!(
                "    let __elm_state_{p} = __elm_ctx.slot({i}usize, || __elm_props.{p}.clone().unwrap_or({d}));\n",
                p = pname,
                i = slot_idx,
                d = render_tokens(d)
            )),
            None => slot_inits.push_str(&format!(
                "    let __elm_state_{p} = __elm_ctx.slot({i}usize, || __elm_props.{p}.clone().unwrap_or_else(|| panic!(\"elm-magic: 필수 prop `{p}`가 없습니다 — mount_with!로 넘기세요\")));\n",
                p = pname,
                i = slot_idx
            )),
        }
        state_names.insert(pname);
        slot_idx += 1;
    }

    // 컴포넌트 children (사양서 3.1) — 항상 prop으로 존재한다.
    props_fields.push_str(
        "    pub children: ::core::option::Option<::std::vec::Vec<::elm_magic::Element>>,\n",
    );
    default_inits.push("children: ::core::option::Option::None".to_string());

    let slot_cell = Cell::new(slot_idx);
    let body_toks: Vec<TokenTree> = body.stream().into_iter().collect();
    // 본문에서 쓰인 식별자(중첩 포함) — 콜백/children/store 사전 바인딩 대상 판정
    let mut used: HashSet<String> = HashSet::new();
    collect_idents(&body_toks, &mut used);

    // store 접근자 사전 바인딩 (사양서 4.2) — 렌더와 이벤트 양쪽에서 쓴다.
    let mut store_map: HashMap<String, store::StoreInfo> = HashMap::new();
    let mut store_prelude = String::new();
    for id in &used {
        if state_names.contains(id) {
            continue;
        }
        if let Some(info) = store::lookup(id) {
            store_prelude.push_str(&format!(
                "    let __elm_store_{i} = {a}::bind(&mut __elm_ctx.arena);\n",
                i = id,
                a = info.accessor
            ));
            store_map.insert(id.clone(), info);
        }
    }

    // 콜백 prop / children 사전 바인딩
    let mut prop_prelude = String::new();
    for cb in &callbacks {
        if used.contains(cb) {
            prop_prelude.push_str(&format!(
                "    let __elm_cb_{c} = __elm_props.{c}.clone().unwrap_or_default();\n",
                c = cb
            ));
        }
    }
    let has_children = used.contains("children") && !state_names.contains("children");
    if has_children {
        prop_prelude.push_str("    let __elm_children_prop = __elm_props.children.clone().unwrap_or_default();\n");
    }

    let env = jsx::Env {
        states: &state_names,
        slots: &slot_cell,
        callbacks: &callbacks,
        has_children,
        stores: &store_map,
    };
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
    // 모든 prop은 `Option<T>`: `None`이면 선언된 기본값을 쓴다 (사양서 4.1).
    code.push_str(&format!(
        "impl ::core::default::Default for {n}Props {{\n    fn default() -> Self {{\n        Self {{\n",
        n = fn_name
    ));
    for f in &default_inits {
        code.push_str(&format!("            {},\n", f));
    }
    code.push_str("        }\n    }\n}\n");
    code.push_str(&format!(
        "{v}struct {n};\nimpl ::elm_magic::Component for {n} {{\n    type Props = {n}Props;\n    const SLOTS: usize = {s};\n    fn render(__elm_ctx: &mut ::elm_magic::Ctx, __elm_props: &{n}Props) -> ::elm_magic::Element {{\n",
        v = vis,
        n = fn_name,
        s = total_slots
    ));
    code.push_str(&slot_inits);
    code.push_str(&store_prelude);
    code.push_str(&prop_prelude);
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

/// Serialize tokens honoring joint punctuation spacing, so `::` stays `::`.
pub(crate) fn render_tokens(toks: &[TokenTree]) -> String {
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

