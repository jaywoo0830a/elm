//! `#[store]` / `store!` — 전역 상태 (사양서 4.2, 10.4).
//!
//! ```ignore
//! #[store]
//! struct App { user: Option<User>, theme: Theme }
//!
//! #[view]
//! fn Header() {
//!     "theme: "{app.theme}
//!     <Button on_click={app.theme = app.theme.clone().toggle()}>"theme"</Button>
//! }
//! ```
//!
//! `#[store]`는 원본 구조체 + 접근자 `<Name>Store`(필드마다 `State<T>`)를 만들고,
//! 인스턴스 이름(`app`)을 **proc-macro 레지스트리**에 등록한다. `view!`는 그
//! 레지스트리를 보고 `app.field`를 `__elm_store_app.field.get(arena)`로 푼다.
//!
//! 전역 상태 슬롯은 아레나에 **이름 기반**으로 저장되므로(사양서 4.2)
//! 컴포넌트 사이에 그대로 공유된다.

use proc_macro::{Delimiter, TokenStream, TokenTree};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn registry() -> &'static Mutex<HashMap<String, StoreInfo>> {
    static REG: OnceLock<Mutex<HashMap<String, StoreInfo>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 스토어 하나의 정보: 접근자 타입 + 필드 이름들.
#[derive(Clone)]
pub struct StoreInfo {
    pub accessor: String,
    pub fields: Vec<String>,
}

fn register(instance: &str, info: StoreInfo) {
    if let Ok(mut m) = registry().lock() {
        m.insert(instance.to_string(), info);
    }
}

/// 인스턴스 이름(`app`) → 스토어 정보.
///
/// proc-macro 크레이트는 rustc 프로세스당 한 번 로드되므로 `#[store]`가
/// `view!`보다 **먼저** 확장될 때 인식된다 (store를 위에 선언하는 자연스러운 순서).
pub fn lookup(instance: &str) -> Option<StoreInfo> {
    registry().lock().ok()?.get(instance).cloned()
}

/// `#[store] struct App { … }`.
pub fn expand_attribute(input: TokenStream) -> TokenStream {
    expand_struct(input.into_iter().collect())
}

/// `store! { App { user: Option<User>, theme: Theme = Theme::Dark } }` —
/// 함수형 형태. 필드 기본값(`field: T = expr`)을 쓸 수 있다.
pub fn expand_fn(input: TokenStream) -> TokenStream {
    let toks: Vec<TokenTree> = input.into_iter().collect();
    // `store! { App { … } }` — `struct` 키워드는 생략 가능
    let mut out: Vec<TokenTree> = Vec::new();
    let mut i = 0;
    if matches!(toks.get(i), Some(TokenTree::Ident(id)) if id.to_string() != "struct") {
        out.push(TokenTree::Ident(proc_macro::Ident::new("struct", toks[i].span())));
    }
    while i < toks.len() {
        out.push(toks[i].clone());
        i += 1;
    }
    expand_struct(out)
}

fn expand_struct(toks: Vec<TokenTree>) -> TokenStream {
    let mut i = 0;
    // `pub` / `pub(crate)` / `pub(super)` / `pub(in path)` — 그룹 내용까지 보존한다.
    let mut vis = String::new();
    if matches!(toks.get(i), Some(TokenTree::Ident(id)) if id.to_string() == "pub") {
        vis = "pub".to_string();
        i += 1;
        if let Some(TokenTree::Group(g)) = toks.get(i) {
            if g.delimiter() == Delimiter::Parenthesis {
                vis.push_str(&format!("({})", g.stream()));
                i += 1;
            }
        }
        vis.push(' ');
    }
    if !matches!(toks.get(i), Some(TokenTree::Ident(id)) if id.to_string() == "struct") {
        panic!("elm-magic: #[store] expects `struct Name {{ field: Type, ... }}`");
    }
    let name = match toks.get(i + 1) {
        Some(TokenTree::Ident(n)) => n.to_string(),
        _ => panic!("elm-magic: #[store] expects `struct Name {{ ... }}`"),
    };
    let body = match toks
        .iter()
        .find(|t| matches!(t, TokenTree::Group(g) if g.delimiter() == Delimiter::Brace))
    {
        Some(TokenTree::Group(g)) => g.clone(),
        _ => panic!("elm-magic: #[store] expects `struct {} {{ ... }}`", name),
    };
    let fields = parse_fields(&name, &body.stream().into_iter().collect::<Vec<_>>());
    if fields.is_empty() {
        panic!("elm-magic: #[store] `{}` needs at least one field", name);
    }

    let instance = to_lower_camel(&name);
    let accessor = format!("{}Store", name);
    register(
        &instance,
        StoreInfo {
            accessor: accessor.clone(),
            fields: fields.iter().map(|(f, _, _)| f.clone()).collect(),
        },
    );

    // 원본 구조체는 기본값(`field: T = expr`)을 떼고 다시 쓴다 —
    // Rust의 "default field values"는 아직 불안정하기 때문.
    let mut original = String::new();
    original.push_str(&format!("{vis}struct {} {{\n", name));
    for (fname, fty, _) in &fields {
        original.push_str(&format!("    pub {}: {},\n", fname, fty));
    }
    original.push_str("}\n");

    let mut accessor_fields = String::new();
    let mut binds = String::new();
    for (fname, fty, fdefault) in &fields {
        accessor_fields.push_str(&format!(
            "    pub {}: ::elm_magic::State<{}>,\n",
            fname, fty
        ));
        let init = match fdefault {
            Some(d) => d.clone(),
            // 기본값이 없으면 `Default::default()` (타입이 Default여야 한다)
            None => "::core::default::Default::default()".to_string(),
        };
        binds.push_str(&format!(
            "            {f}: __elm_a.store(\"{n}.{f}\", || {init}),\n",
            f = fname,
            n = name,
            init = init
        ));
    }

    let code = format!(
        "#[allow(dead_code)]\n{original}\n\
         /// `{name}` 전역 상태 접근자 (사양서 4.2) — `{instance}.field`가 이걸로 풀린다.\n\
         #[derive(::core::clone::Clone)]\n\
         {vis}struct {accessor} {{\n{accessor_fields}}}\n\
         impl {accessor} {{\n\
         \x20   /// 아레나에서 store 슬롯을 얻는다 (첫 호출에 초기값).\n\
         \x20   pub fn bind(__elm_a: &mut ::elm_magic::Arena) -> Self {{\n\
         \x20       Self {{\n{binds}        }}\n    }}\n}}\n",
        original = original,
        name = name,
        instance = instance,
        accessor = accessor,
        accessor_fields = accessor_fields,
        binds = binds,
        vis = vis,
    );
    code.parse()
        .unwrap_or_else(|e| panic!("elm-magic internal: bad store code for `{}`: {:?}", name, e))
}

fn parse_fields(name: &str, toks: &[TokenTree]) -> Vec<(String, String, Option<String>)> {
    let mut out = Vec::new();
    for field in split_commas(toks) {
        let fname = match field.first() {
            Some(TokenTree::Ident(id)) => id.to_string(),
            _ => continue,
        };
        let mut k = 1;
        let mut fty = String::new();
        let mut fdefault = None;
        if matches!(field.get(k), Some(TokenTree::Punct(p)) if p.as_char() == ':') {
            k += 1;
            let mut ty = Vec::new();
            while k < field.len() {
                if matches!(&field[k], TokenTree::Punct(p) if p.as_char() == '=') {
                    break;
                }
                ty.push(field[k].clone());
                k += 1;
            }
            fty = ty
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(" ")
                .replace(" :: ", "::");
        }
        if k < field.len() {
            fdefault = Some(crate::view::render_tokens(&field[k + 1..]));
        }
        if fty.is_empty() {
            panic!(
                "elm-magic: store field `{}.{}` needs a type annotation",
                name, fname
            );
        }
        out.push((fname, fty, fdefault));
    }
    out
}

fn split_commas(toks: &[TokenTree]) -> Vec<Vec<TokenTree>> {
    let mut out = Vec::new();
    let mut cur: Vec<TokenTree> = Vec::new();
    let mut depth = 0i32;
    for t in toks {
        if let TokenTree::Punct(p) = t {
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
        cur.push(t.clone());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `App` → `app`, `AppState` → `appState`.
fn to_lower_camel(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_lowercase(), chars.as_str()),
        None => name.to_string(),
    }
}
