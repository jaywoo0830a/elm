use proc_macro::{Delimiter, TokenStream, TokenTree};
use std::sync::atomic::{AtomicUsize, Ordering};

static CSS_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Parse `css!` input: `.sel { key: value; ... }` or `tag { ... }`.
///
/// Emits items that self-register at process startup (`.init_array` ctor),
/// so `css!` can be used at item position like the spec shows (사양서 6.1).
/// Platform note: ctor registration works on Linux/macOS; wasm will need a
/// different mechanism (v0.3+).
pub fn expand(input: TokenStream) -> TokenStream {
    let toks: Vec<TokenTree> = input.into_iter().collect();
    let mut i = 0;
    let mut entries: Vec<String> = Vec::new();

    while i < toks.len() {
        // selector: `.name` or bare tag ident
        let selector = match (&toks[i], toks.get(i + 1)) {
            (TokenTree::Punct(dot), Some(TokenTree::Ident(id))) if dot.as_char() == '.' => {
                i += 2;
                format!(".{}", id)
            }
            (TokenTree::Ident(id), _) => {
                i += 1;
                id.to_string()
            }
            other => panic!("elm-magic css!: unexpected selector token {:?}", other),
        };
        let group = match toks.get(i) {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => g,
            other => panic!("elm-magic css!: expected {{ after selector: {:?}", other),
        };
        i += 1;

        let mut props: Vec<(String, String)> = Vec::new();
        let mut cur: Vec<String> = Vec::new();
        for t in group.stream() {
            match &t {
                TokenTree::Punct(p) if p.as_char() == ';' => flush_prop(&mut cur, &mut props),
                TokenTree::Literal(l) => cur.push(l.to_string()),
                TokenTree::Ident(id) => cur.push(id.to_string()),
                TokenTree::Punct(p) => cur.push(p.as_char().to_string()),
                _ => panic!("elm-magic css!: unsupported token in declaration"),
            }
        }
        flush_prop(&mut cur, &mut props);

        let props_str = props
            .iter()
            .map(|(k, v)| format!("({:?}, {:?})", k, v))
            .collect::<Vec<_>>()
            .join(", ");
        entries.push(format!("({:?}, vec![{}])", selector, props_str));
    }

    let n = CSS_COUNT.fetch_add(1, Ordering::Relaxed);
    let code = format!(
        "extern \"C\" fn __elm_css_register_{n}() {{ \
            ::elm_magic::style::register(vec![{}]); \
        }} \
        #[used] \
        #[link_section = \".init_array\"] \
        static __ELM_CSS_CTOR_{n}: extern \"C\" fn() = __elm_css_register_{n};",
        entries.join(", ")
    );
    code.parse().expect("elm-magic css!: bad generated code")
}

/// Flush the accumulated tokens of one declaration into `props`.
/// Tokens are space-joined, then split on the first `:`.
fn flush_prop(cur: &mut Vec<String>, props: &mut Vec<(String, String)>) {
    if !cur.is_empty() {
        let joined = cur.join(" ");
        if let Some(ci) = joined.find(':') {
            let k = joined[..ci].trim();
            let v = joined[ci + 1..].trim();
            if !k.is_empty() {
                props.push((k.to_string(), v.to_string()));
            }
        }
    }
    cur.clear();
}
