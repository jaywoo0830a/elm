use proc_macro::{Delimiter, Group, TokenStream, TokenTree};
use std::sync::atomic::{AtomicUsize, Ordering};

static CSS_COUNT: AtomicUsize = AtomicUsize::new(0);

/// 값의 종류 — 컴파일타임 검증에 쓴다.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// 숫자 하나 (`gap: 16`)
    Len,
    /// 상하좌우 숏핸드 (`padding: 8 16`)
    Edges,
    /// 팔레트 토큰 (`bg: surface`)
    Color,
    /// 키워드 (`weight: bold`)
    Keyword,
}

/// (속성 이름, `StyleSpec` 필드, 값 종류).
///
/// 키워드 속성은 값에 따라 만들 필드가 달라서 필드 이름을 비워 둔다
/// (`emit_field`가 처리한다).
const PROPS: &[(&str, &str, Kind)] = &[
    ("gap", "gap", Kind::Len),
    ("padding", "padding", Kind::Edges),
    ("margin", "margin", Kind::Edges),
    ("bg", "bg", Kind::Color),
    ("color", "color", Kind::Color),
    ("radius", "radius", Kind::Len),
    ("font-size", "font_size", Kind::Len),
    ("weight", "", Kind::Keyword),
    ("text-decoration", "", Kind::Keyword),
];

/// (토큰 이름, `Token` variant).
///
/// `src/style.rs::Token::ALL`과 같아야 한다 —
/// `tests/css.rs::css_token_vocabulary_matches_core`가 이 일치를 지킨다.
const TOKENS: &[(&str, &str)] = &[
    ("primary", "Primary"),
    ("on_primary", "OnPrimary"),
    ("surface", "Surface"),
    ("background", "Background"),
    ("text", "Text"),
    ("text_dim", "TextDim"),
    ("error", "Error"),
    ("warn", "Warn"),
];

/// Parse `css!` input: `.sel { key: value; ... }` or `tag { ... }` (사양서 6.1).
///
/// **컴파일타임에 검증한다** — 모르는 속성, 잘못된 값 종류, 같은 `css!` 안의
/// 중복 셀렉터/속성은 모두 즉시 컴파일 에러다 (조용히 버리지 않는다).
///
/// Emits items that self-register at process startup (`.init_array` ctor),
/// so `css!` can be used at item position like the spec shows.
///
/// Platform note (실측, 2026-09): ELF/Mach-O 전용이다.
/// - 최종 바이너리가 그 크레이트의 심볼을 **하나라도** 참조하면 (다른 모듈·다른
///   codegen unit이어도) 등록이 실행된다 — 라이브러리에서 `css!`를 써도 된다.
/// - 아무것도 참조하지 않으면 링커가 rlib 객체를 포함하지 않아 등록되지 않는다
///   (안 쓰는 라이브러리 = 스타일도 없음 — 의도된 결과).
/// - wasm은 다른 메커니즘이 필요하다.
pub fn expand(input: TokenStream) -> TokenStream {
    let toks: Vec<TokenTree> = input.into_iter().collect();
    let mut i = 0;
    let mut rules: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    while i < toks.len() {
        // selector: `.name`(클래스) 또는 bare tag ident(태그)
        let (selector, is_class) = match (&toks[i], toks.get(i + 1)) {
            (TokenTree::Punct(dot), Some(TokenTree::Ident(id))) if dot.as_char() == '.' => {
                i += 2;
                (format!(".{}", id), true)
            }
            (TokenTree::Ident(id), _) => {
                i += 1;
                (id.to_string(), false)
            }
            other => panic!("elm-magic css!: 셀렉터가 필요합니다 (`.이름` 또는 태그): {:?}", other),
        };
        let group = match toks.get(i) {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => g.clone(),
            other => panic!("elm-magic css!: `{selector}` 뒤에 `{{ … }}`가 필요합니다: {:?}", other),
        };
        i += 1;

        // 태그는 대소문자를 무시한다 (`Button` ≡ `button`).
        let key = if is_class { selector.clone() } else { selector.to_lowercase() };
        if seen.contains(&key) {
            panic!("elm-magic css!: 셀렉터 `{selector}`가 같은 `css!`에서 두 번 선언됐습니다");
        }
        seen.push(key);

        let fields = parse_rule(&selector, group);
        rules.push(emit_rule(&selector, &fields));
    }

    let n = CSS_COUNT.fetch_add(1, Ordering::Relaxed);
    let code = format!(
        "extern \"C\" fn __elm_css_register_{n}() {{ \
            ::elm_magic::style::register(::std::vec![{}]); \
        }} \
        #[used] \
        #[link_section = \".init_array\"] \
        static __ELM_CSS_CTOR_{n}: extern \"C\" fn() = __elm_css_register_{n};",
        rules.join(", ")
    );
    code.parse().expect("elm-magic css!: 잘못된 코드 생성")
}

/// 한 규칙의 `key: value;` 들을 `StyleSpec` 필드 초기화 목록으로 바꾼다.
fn parse_rule(selector: &str, group: Group) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    let mut declared: Vec<String> = Vec::new();
    let mut cur: Vec<String> = Vec::new();

    for t in group.stream() {
        match &t {
            TokenTree::Punct(p) if p.as_char() == ';' => {
                flush(selector, &mut cur, &mut declared, &mut fields);
            }
            TokenTree::Literal(l) => cur.push(l.to_string()),
            TokenTree::Ident(id) => cur.push(id.to_string()),
            TokenTree::Punct(p) => cur.push(p.as_char().to_string()),
            _ => panic!("elm-magic css!: `{selector}`의 선언에 쓸 수 없는 토큰이 있습니다"),
        }
    }
    flush(selector, &mut cur, &mut declared, &mut fields);
    fields
}

/// 쌓아 둔 토큰 하나(`key: value`)를 필드로 바꾼다. 빈 선언은 무시한다.
fn flush(
    selector: &str,
    cur: &mut Vec<String>,
    declared: &mut Vec<String>,
    fields: &mut Vec<String>,
) {
    if cur.is_empty() {
        return;
    }
    let joined = glue(&cur.join(" "));
    cur.clear();
    let Some(colon) = joined.find(':') else {
        panic!("elm-magic css!: `{selector}`의 선언 `{joined}`에 `:`가 없습니다");
    };
    let name = joined[..colon].trim().to_string();
    let value = joined[colon + 1..].trim().to_string();
    if name.is_empty() {
        panic!("elm-magic css!: `{selector}`에 속성 이름이 없습니다 (`{joined}`)");
    }
    let Some((_, field, kind)) = PROPS.iter().find(|(n, _, _)| *n == name) else {
        panic!(
            "elm-magic css!: 모르는 속성 `{name}` (셀렉터 `{selector}`). 지원: {}",
            PROPS.iter().map(|(n, _, _)| *n).collect::<Vec<_>>().join(", ")
        );
    };
    if declared.contains(&name) {
        panic!("elm-magic css!: `{selector}`에 `{name}`가 두 번 있습니다");
    }
    declared.push(name.clone());
    fields.extend(emit_field(&name, field, *kind, &value, selector));
}

/// 속성 하나 → `StyleSpec` 필드 초기화 문자열(들).
fn emit_field(name: &str, field: &str, kind: Kind, value: &str, selector: &str) -> Vec<String> {
    match kind {
        Kind::Len => {
            let v = parse_len(value, name, selector);
            vec![format!("{field}: ::std::option::Option::Some({v}f32)")]
        }
        Kind::Edges => {
            let v = parse_edges(value, name, selector);
            vec![format!("{field}: ::std::option::Option::Some({v})")]
        }
        Kind::Color => {
            let Some((_, variant)) = TOKENS.iter().find(|(t, _)| *t == value) else {
                panic!(
                    "elm-magic css!: `{selector}`의 `{name}` 값 `{value}`는 팔레트 토큰이 아닙니다. 지원: {}",
                    TOKENS.iter().map(|(t, _)| *t).collect::<Vec<_>>().join(", ")
                );
            };
            vec![format!(
                "{field}: ::std::option::Option::Some(::elm_magic::style::Token::{variant})"
            )]
        }
        Kind::Keyword => match (name, value) {
            ("weight", "bold") => vec![some("bold", true)],
            ("weight", "normal") => vec![some("bold", false)],
            ("text-decoration", "line-through") => {
                vec![some("strike", true), some("underline", false)]
            }
            ("text-decoration", "underline") => {
                vec![some("strike", false), some("underline", true)]
            }
            ("text-decoration", "none") => {
                vec![some("strike", false), some("underline", false)]
            }
            _ => panic!(
                "elm-magic css!: `{selector}`의 `{name}` 값 `{value}`는 쓸 수 없습니다 ({})",
                keyword_values(name)
            ),
        },
    }
}

/// `Option<bool>` 필드 초기화 문자열.
fn some(field: &str, value: bool) -> String {
    format!("{field}: ::std::option::Option::Some({value})")
}

/// 키워드 속성이 받는 값 목록 (에러 메시지용).
fn keyword_values(name: &str) -> &'static str {
    match name {
        "weight" => "bold, normal",
        _ => "none, line-through, underline",
    }
}

/// 숫자 하나 (`16`, `16px`, `1.5`, `-8`).
fn parse_len(value: &str, name: &str, selector: &str) -> f32 {
    let t = value.trim();
    let t = t.strip_suffix("px").unwrap_or(t).trim();
    t.parse::<f32>().unwrap_or_else(|_| {
        panic!(
            "elm-magic css!: `{selector}`의 `{name}` 값 `{value}`는 숫자가 아닙니다 (예: `16`, `16px`, `1.5`)"
        )
    })
}

/// 1·2·4개 숫자 → `Edges` 생성 코드.
fn parse_edges(value: &str, name: &str, selector: &str) -> String {
    let mut nums: Vec<f32> = Vec::new();
    let mut negative = false;
    for word in value.split_whitespace() {
        if word == "-" {
            negative = true;
            continue;
        }
        let v = parse_len(word, name, selector);
        nums.push(if negative { -v } else { v });
        negative = false;
    }
    match nums.len() {
        1 => format!("::elm_magic::style::Edges::splat({}f32)", nums[0]),
        2 => format!(
            "::elm_magic::style::Edges::symmetric({}f32, {}f32)",
            nums[0], nums[1]
        ),
        4 => format!(
            "::elm_magic::style::Edges::new({}f32, {}f32, {}f32, {}f32)",
            nums[0], nums[1], nums[2], nums[3]
        ),
        _ => panic!(
            "elm-magic css!: `{selector}`의 `{name}`은 1·2·4개의 숫자를 받습니다 (예: `16`, `8 16`, `8 16 8 16`) — `{value}`"
        ),
    }
}

/// 토큰을 이어 붙일 때 **식별자 사이의 하이픈**만 붙인다.
///
/// `font - size` → `font-size` (속성 이름/키워드),
/// `8 - 16` → `8 - 16` (숫자 부호는 그대로 둔다).
fn glue(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let word = |c: char| c.is_alphabetic() || c == '_';
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '-' {
            let left = out.trim_end().chars().next_back().map(word).unwrap_or(false);
            let mut j = i + 1;
            while cs.get(j) == Some(&' ') {
                j += 1;
            }
            let right = cs.get(j).copied().map(word).unwrap_or(false);
            if left && right {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push('-');
                i = j;
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

/// 한 규칙 → `register` 항목 하나.
fn emit_rule(selector: &str, fields: &[String]) -> String {
    let fields: String = fields.iter().map(|f| format!("{f}, ")).collect();
    format!(
        "({selector:?}, ::elm_magic::style::StyleSpec {{ {fields}..::elm_magic::style::StyleSpec::NONE }})"
    )
}
