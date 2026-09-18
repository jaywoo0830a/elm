use proc_macro::{Delimiter, Group, TokenStream, TokenTree};
use std::sync::atomic::{AtomicUsize, Ordering};

static CSS_COUNT: AtomicUsize = AtomicUsize::new(0);

/// 플랫폼별 **시작 초기화 섹션** 속성 (0.7.4 — 리포트 버그 13).
///
/// `.init_array`는 ELF 전용이라 MSVC(PE/COFF)에서는 실행되지 않는다.
/// MSVC의 CRT는 `.CRT$XCU`를, Apple은 `__DATA,__mod_init_func`를 걷는다.
/// 셋 중 **정확히 하나만** 활성화되도록 `cfg_attr`로 분기한다.
const CTOR_SECTION_ATTRS: &str = "\
    #[cfg_attr(target_env = \"msvc\", link_section = \".CRT$XCU\")] \
    #[cfg_attr(target_vendor = \"apple\", link_section = \"__DATA,__mod_init_func\")] \
    #[cfg_attr( \
        not(any(target_env = \"msvc\", target_vendor = \"apple\")), \
        link_section = \".init_array\" \
    )]";

/// 값의 종류 — 컴파일타임 검증에 쓴다.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// 숫자 하나 (`gap: 16`)
    Num,
    /// 길이: 숫자 | `fill` | `auto` (`width: 200`)
    Len,
    /// 상하우 숏드 (`padding: 8 16`)
    Edges,
    /// 팔레트 토큰 (`bg: surface`)
    Color,
    /// 정렬 (`align: center`)
    Align,
    /// `true` / `false`
    Bool,
    /// `none` / `flex`
    Display,
    /// `visible` / `hidden`
    Visibility,
    /// `bold` / `normal`
    Weight,
    /// `italic` / `normal`
    Style,
    /// `monospace` / `proportional`
    Family,
    /// `none` / `line-through` / `underline`
    Decoration,
    /// `none` / `uppercase` / `lowercase` / `capitalize`
    Transform,
    /// 커서 이름
    Cursor,
    /// `x y blur spread` (1~4개 숫자)
    Shadow,
}

/// (속성 이름, `StyleSpec` 필드, 값 종류).
///
/// 키워드 속성은 값에 따라 만들 필드가 달라서 필드 이름을 비워 둔다
/// (`emit_field`가 처리한다).
const PROPS: &[(&str, &str, Kind)] = &[
    ("gap", "gap", Kind::Num),
    ("row-gap", "row_gap", Kind::Num),
    ("column-gap", "column_gap", Kind::Num),
    ("padding", "padding", Kind::Edges),
    ("margin", "margin", Kind::Edges),
    ("width", "width", Kind::Len),
    ("height", "height", Kind::Len),
    ("min-width", "min_width", Kind::Num),
    ("min-height", "min_height", Kind::Num),
    ("max-width", "max_width", Kind::Num),
    ("max-height", "max_height", Kind::Num),
    ("align", "align", Kind::Align),
    ("justify", "justify", Kind::Align),
    ("wrap", "wrap", Kind::Bool),
    ("display", "display_none", Kind::Display),
    ("visibility", "hidden", Kind::Visibility),
    ("bg", "bg", Kind::Color),
    ("fill", "fill", Kind::Color),
    ("border-width", "border_width", Kind::Num),
    ("border-color", "border_color", Kind::Color),
    ("radius", "radius", Kind::Num),
    ("shadow", "shadow", Kind::Shadow),
    ("shadow-color", "shadow_color", Kind::Color),
    ("opacity", "opacity", Kind::Num),
    ("color", "color", Kind::Color),
    ("font-size", "font_size", Kind::Num),
    ("line-height", "line_height", Kind::Num),
    ("letter-spacing", "letter_spacing", Kind::Num),
    ("weight", "bold", Kind::Weight),
    ("font-style", "italic", Kind::Style),
    ("font-family", "mono", Kind::Family),
    ("text-decoration", "", Kind::Decoration),
    ("text-align", "text_align", Kind::Align),
    ("text-transform", "transform", Kind::Transform),
    ("truncate", "truncate", Kind::Bool),
    ("cursor", "cursor", Kind::Cursor),
];

/// (토큰 이름, `Token` variant).
///
/// `src/style.rs::Token::ALL`과 같아야 한다 —
/// `tests/css.rs::css_token_vocabulary_matches_core`가 이 일치를 지킨다.
const TOKENS: &[(&str, &str)] = &[
    ("primary", "Primary"),
    ("on_primary", "OnPrimary"),
    ("surface", "Surface"),
    ("surface_alt", "SurfaceAlt"),
    ("background", "Background"),
    ("text", "Text"),
    ("text_dim", "TextDim"),
    ("error", "Error"),
    ("warn", "Warn"),
    ("success", "Success"),
    ("info", "Info"),
    ("border", "Border"),
    ("shadow", "Shadow"),
    ("overlay", "Overlay"),
];

/// Parse `css!` input.
///
/// 문법: `셀렉터 { 속성: 값; … }` — 렉터는
/// `button` `.card` `*` `.card.muted` `.card Button` `Col > Row` `.a, .b`
/// `button:hover` `:disabled` 등을 받는다.
///
/// **컴파일타임에 검증한다** — 깨진 셀터, 모르는 속성/값, 한 블록 안의
/// 중복은 모두 즉시 컴파일 에러다 (조용히 버리지 않는다).
///
/// Emits items that self-register at process startup (ctor in the platform's
/// init section), so `css!` can be used at item position like the spec shows.
///
/// Platform note (0.7.4 — 리포트 버그 13):
/// - ELF → `.init_array`, Apple(Mach-O) → `__DATA,__mod_init_func`,
///   Windows MSVC(PE/COFF) → `.CRT$XCU`. MSVC의 CRT는 `.init_array`를 실행하지
///   않으므로, 예전에는 Windows 빌드에서 `style::len() == 0`이 되어 모든
///   `class="…"`가 무시됐다.
/// - 최종 바이너리가 그 크레이트의 심볼을 **하나라도** 참조하면 (다른 모듈·다른
///   codegen unit이어도) 등록이 실행된다 — 라이브러리에서 `css!`를 써도 된다.
/// - 아무것도 참조하지 않으면 링커가 rlib 객체를 포함하지 않아 등록되지 않는다
///   (안 쓰는 라이브러리 = 스타일도 없음 — 의도된 결과).
/// - wasm은 다른 메커니즘이 필요하다 — `style::init_styles()`로 다시 적용할 수
///   있지만 그마저도 시작 등록이 한 번은 돌아야 한다.
pub fn expand(input: TokenStream) -> TokenStream {
    let toks: Vec<TokenTree> = input.into_iter().collect();
    let mut i = 0;
    let mut rules: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    while i < toks.len() {
        // 1) `{` 그룹이 나올 때까지가 셀렉터
        let mut pieces: Vec<String> = Vec::new();
        let group = loop {
            match toks.get(i) {
                Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                    let g = g.clone();
                    i += 1;
                    break g;
                }
                Some(TokenTree::Punct(p)) => pieces.push(p.as_char().to_string()),
                Some(TokenTree::Ident(id)) => pieces.push(id.to_string()),
                Some(TokenTree::Literal(l)) => pieces.push(l.to_string()),
                Some(other) => {
                    panic!("elm-magic css!: 셀렉터에 쓸 수 없는 토큰입니다: {other}")
                }
                None => panic!("elm-magic css!: 렉터 뒤에 `{{ … }}`가 필요합니다"),
            }
            i += 1;
        };
        let selector = if pieces.len() == 1 {
            // 문자열 리터럴 하나면 **공백까지 그대로** 쓴다 (진짜 CSS)
            match unquote(&pieces[0]) {
                Some(raw) => raw,
                None => join_selector(&pieces),
            }
        } else {
            join_selector(&pieces)
        };
        validate_selector(&selector);

        let fields = parse_rule(&selector, group);
        // 셀렉터 목록(`.a, .b`)은 같은 선언을 각각 등록한다
        for one in selector.split(',') {
            let one = one.trim();
            if seen.iter().any(|s| s == one) {
                panic!("elm-magic css!: 렉터 `{one}`가 같은 `css!`에서 두 번 선언됐습니다");
            }
            seen.push(one.to_string());
            rules.push(emit_rule(one, &fields));
        }
    }

    let n = CSS_COUNT.fetch_add(1, Ordering::Relaxed);
    let code = format!(
        "extern \"C\" fn __elm_css_register_{n}() {{ \
            ::elm_magic::style::register(::std::vec![{}]); \
        }} \
        #[used] \
        {section} \
        static __ELM_CSS_CTOR_{n}: extern \"C\" fn() = __elm_css_register_{n};",
        rules.join(", "),
        section = CTOR_SECTION_ATTRS
    );
    code.parse().expect("elm-magic css!: 잘된 코드 생성")
}

#[cfg(test)]
mod tests {
    /// 플랫폼별 시작 섹션이 **셋 다** 들어 있고 서로 배타적인지 고정한다
    /// (0.7.4 — 리포트 버그 13: MSVC에서 등록이 안 돌던 문제).
    #[test]
    fn ctor_section_attrs_cover_elf_macho_and_msvc() {
        let a = super::CTOR_SECTION_ATTRS;
        assert!(a.contains("target_env = \"msvc\""), "{a}");
        assert!(a.contains(".CRT$XCU"), "{a}");
        assert!(a.contains("target_vendor = \"apple\""), "{a}");
        assert!(a.contains("__DATA,__mod_init_func"), "{a}");
        assert!(a.contains("not(any(target_env = \"msvc\", target_vendor = \"apple\"))"));
        assert!(a.contains(".init_array"), "{a}");
    }
}

/// 문자열 리터럴이면 따옴표를 걷어 내용을 돌려준다 (`"…"`, `r"…"`, `r#"…"#`).
fn unquote(piece: &str) -> Option<String> {
    let body = piece.strip_prefix('r').unwrap_or(piece);
    let start = body.find('"')?;
    let end = body.rfind('"')?;
    if end <= start {
        return None;
    }
    Some(body[start + 1..end].to_string())
}

/// 셀렉터 토큰들을 잇는다.
///
/// **식별자(또는 `*`) 사이만** 공백을 넣는다 —
/// `.card Button`은 후손(공백), `.card>Button`은 자식, `.card.muted`는 복합.
///
/// 하이픈 조각(`-`)은 단어가 아니라 **앞 식별자에 이어 붙는 접합자**다 —
/// `.tabs__item--active`가 `.tabs__item - - active`로, `.my-class`가
/// `.my - class`로 조인되면 코어 파싱이 실패해 **조용히 미등록**된다 (0.6.1에서 수정).
///
/// Rust 토큰은 공백을 보존하지 않으므로 **클래스 사이 후손**(`.a .b`)은
/// 구분할 수 없다 — 그런 셀렉터는 문자열로 쓴다: `".a .b" { … }`.
fn join_selector(pieces: &[String]) -> String {
    let word =
        |s: &str| s == "*" || (!s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_'));
    let hyphen = |s: &str| !s.is_empty() && s.chars().all(|c| c == '-');
    let mut out = String::new();
    let mut prev_word = false;
    for piece in pieces {
        // `-`는 앞 식별자에 그대로 붙이고, 뒤 식별자도 공백 없이 따라오게 한다
        if hyphen(piece) && prev_word {
            out.push_str(piece);
            prev_word = false;
            continue;
        }
        let is_word = word(piece);
        if is_word && prev_word {
            out.push(' ');
        }
        out.push_str(piece);
        prev_word = is_word;
    }
    out
}

/// 렉터 문법을 검증한다 (core `Selector::parse`와 같은 규칙).
///
/// 코어가 거부하는 셀렉터는 등록되지 않으므로(0.6.0까지는 조용히, 0.6.1부터는
/// panic), 흔한 실수는 여기서 **컴파일 에러**로 만든다.
fn validate_selector(selector: &str) {
    if selector.trim().is_empty() {
        panic!("elm-magic css!: 렉터가 비었습니다");
    }
    for one in selector.split(',') {
        let one = one.trim();
        if one.is_empty() {
            panic!("elm-magic css!: 터 목록에  항목이 있습니다 (`{selector}`)");
        }
        if one.starts_with('>') || one.ends_with('>') || one.contains(">>") {
            panic!("elm-magic css!: `{one}`의 결합자 `>` 위치가 잘못됐습니다");
        }
        let mut rest = one;
        while let Some(pos) = rest.find(':') {
            rest = &rest[pos + 1..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            match name.as_str() {
                "hover" | "active" | "focus" | "focused" | "disabled" => {}
                "" => panic!("elm-magic css!: `{one}`에 `:` 뒤 상태 이름이 없습니다"),
                other => panic!(
                    "elm-magic css!: 모르는 상태 `:{other}` (셀렉터 `{one}`). 지원: hover, active, focus, disabled"
                ),
            }
            rest = &rest[name.len()..];
        }
        // 코어가 파싱하지 못하는 셀렉터는 등록 루틴이 조용히 버린다 —
        // "스타일이 안 먹는다"로만 보이므로 컴파일 에러로 승격한다 (0.6.1).
        if !selector_is_valid(one) {
            panic!(
                "elm-magic css!: `{one}`의 셀렉터 문법이 잘못됐습니다. \
                 마디는 `태그` / `*` / `.클래스` / `:상태`의 조합이고, \
                 클래스 이름은 `.` 뒤에 와야 합니다 (예: `.tabs__item--active`)"
            );
        }
    }
}

/// 셀렉터 하나(결합자 포함)를 코어 `Selector::parse`와 **같은 방식**으로
/// 쪼개 마디마다 검사한다.
///
/// 매크로는 코어에 의존할 수 없어(순환 의존) 규칙을 옮겨 적었다.
fn selector_is_valid(one: &str) -> bool {
    let mut rest = one.trim();
    if rest.is_empty() {
        return false;
    }
    let mut count = 0usize;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('>') {
            if count == 0 {
                return false; // `>`로 시작할 수 없다
            }
            rest = r.trim_start();
            if rest.is_empty() {
                return false;
            }
        }
        let end = rest
            .find(|c: char| c == '>' || c.is_whitespace())
            .unwrap_or(rest.len());
        if !compound_is_valid(&rest[..end]) {
            return false;
        }
        count += 1;
        rest = rest[end..].trim_start();
    }
    count > 0
}

/// 마디 하나(`.card`, `Button:hover`, `*`)를 코어 `parse_compound`와 같은
/// 규칙으로 검사한다.
fn compound_is_valid(text: &str) -> bool {
    let cs: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut seen = false;
    let mut tag_seen = false;
    while i < cs.len() {
        match cs[i] {
            '.' => {
                i += 1;
                if ident(&cs, &mut i).is_empty() {
                    return false; // `.` 뒤에 이름이 없다
                }
                seen = true;
            }
            ':' => {
                i += 1;
                if !matches!(
                    ident(&cs, &mut i).as_str(),
                    "hover" | "active" | "focus" | "focused" | "disabled"
                ) {
                    return false;
                }
                seen = true;
            }
            '*' => {
                if seen {
                    return false; // `*`는 마디에 혼자만 온다
                }
                i += 1;
                tag_seen = true;
                seen = true;
            }
            c if c.is_alphanumeric() || c == '_' => {
                if tag_seen {
                    return false; // 태그는 마디에 하나만
                }
                ident(&cs, &mut i);
                tag_seen = true;
                seen = true;
            }
            _ => return false,
        }
    }
    seen
}

/// 식별자 하나를 읽는다 (코어 `style::ident`와 같다 — `-` 포함).
fn ident(cs: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < cs.len() && (cs[*i].is_alphanumeric() || cs[*i] == '_' || cs[*i] == '-') {
        *i += 1;
    }
    cs[start..*i].iter().collect()
}

/// 한 규칙의 `key: value;` 들을 `StyleSpec` 필드 초기화 목록으로 바다.
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
            PROPS
                .iter()
                .map(|(n, _, _)| *n)
                .collect::<Vec<_>>()
                .join(", ")
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
    let some = |expr: String| vec![format!("{field}: ::std::option::Option::Some({expr})")];
    // `text-decoration`만 값을 필드 **두 개**로 나다
    if kind == Kind::Decoration {
        return match value {
            "none" => vec![field_of("strike", "false"), field_of("underline", "false")],
            "line-through" | "strikethrough" => {
                vec![field_of("strike", "true"), field_of("underline", "false")]
            }
            "underline" => vec![field_of("strike", "false"), field_of("underline", "true")],
            other => panic!(
                "elm-magic css!: `{selector}`의 `{name}` 값 `{other}`는 쓸 수 없습니다 (none, line-through, underline)"
            ),
        };
    }
    match kind {
        Kind::Num => some(format!("{}f32", parse_num(value, name, selector))),
        Kind::Len => some(match value {
            "fill" | "full" | "100%" => "::elm_magic::style::Len::Fill".to_string(),
            "auto" => "::elm_magic::style::Len::Auto".to_string(),
            _ => format!(
                "::elm_magic::style::Len::Px({}f32)",
                parse_num(value, name, selector)
            ),
        }),
        Kind::Edges => some(parse_edges(value, name, selector)),
        Kind::Color => some(format!(
            "::elm_magic::style::Token::{}",
            token_variant(value, name, selector)
        )),
        Kind::Align => some(format!(
            "::elm_magic::style::Align::{}",
            align_variant(value, name, selector)
        )),
        Kind::Bool => some(bool_expr(value, name, selector, "true", "false")),
        Kind::Display => some(bool_expr(value, name, selector, "none", "flex")),
        Kind::Visibility => some(bool_expr(value, name, selector, "hidden", "visible")),
        Kind::Weight => some(bool_expr(value, name, selector, "bold", "normal")),
        Kind::Style => some(bool_expr(value, name, selector, "italic", "normal")),
        Kind::Family => some(bool_expr(
            value,
            name,
            selector,
            "monospace",
            "proportional",
        )),
        Kind::Transform => some(format!(
            "::elm_magic::style::Transform::{}",
            transform_variant(value, name, selector)
        )),
        Kind::Cursor => some(format!(
            "::elm_magic::style::Cursor::{}",
            cursor_variant(value, name, selector)
        )),
        Kind::Shadow => some(shadow_expr(value, name, selector)),
        Kind::Decoration => unreachable!(),
    }
}

/// `field: Some(value)` 문자열.
fn field_of(field: &str, value: &str) -> String {
    format!("{field}: ::std::option::Option::Some({value})")
}

/// 키워드 짝(`yes`/`no`) → `true`/`false`.
fn bool_expr(value: &str, name: &str, selector: &str, yes: &str, no: &str) -> String {
    if value == yes {
        "true".to_string()
    } else if value == no {
        "false".to_string()
    } else {
        panic!(
            "elm-magic css!: `{selector}`의 `{name}` 값 `{value}`는 쓸 수 없습니다 ({yes}, {no})"
        )
    }
}

/// 팔레트 토큰 이름 → variant (`src/style.rs::TOKENS`와 같아야 한다).
fn token_variant(value: &str, name: &str, selector: &str) -> &'static str {
    match TOKENS.iter().find(|(t, _)| *t == value) {
        Some((_, variant)) => variant,
        None => {
            panic!(
            "elm-magic css!: `{selector}`의 `{name}` 값 `{value}`는 레트 토큰이 아닙니다. 지원: {}",
            TOKENS.iter().map(|(t, _)| *t).collect::<Vec<_>>().join(", ")
        )
        }
    }
}

/// 정렬 값 → variant.
fn align_variant(value: &str, name: &str, selector: &str) -> &'static str {
    match value {
        "start" | "left" | "top" | "flex-start" => "Start",
        "center" => "Center",
        "end" | "right" | "bottom" | "flex-end" => "End",
        other => panic!(
            "elm-magic css!: `{selector}`의 `{name}` 값 `{other}`는 쓸 수 없습니다 (start, center, end)"
        ),
    }
}

/// `text-transform` 값 → variant.
fn transform_variant(value: &str, name: &str, selector: &str) -> &'static str {
    match value {
        "none" => "None",
        "uppercase" | "upper" => "Upper",
        "lowercase" | "lower" => "Lower",
        "capitalize" => "Capitalize",
        other => panic!(
            "elm-magic css!: `{selector}`의 `{name}` 값 `{other}`는 쓸 수 없습니다 (none, uppercase, lowercase, capitalize)"
        ),
    }
}

/// 커서 값 → variant.
fn cursor_variant(value: &str, name: &str, selector: &str) -> &'static str {
    match value {
        "default" | "auto" => "Default",
        "pointer" | "hand" | "click" => "Pointer",
        "text" | "ibeam" => "Text",
        "grab" => "Grab",
        "grabbing" => "Grabbing",
        "move" | "all-scroll" => "Move",
        "crosshair" => "Crosshair",
        "not-allowed" | "no-drop" => "NotAllowed",
        "none" => "None",
        other => panic!(
            "elm-magic css!: `{selector}`의 `{name}` 값 `{other}`는 모르는 커서입니다 (default, pointer, text, grab, grabbing, move, crosshair, not-allowed, none)"
        ),
    }
}

/// 숫자 하나 (`16`, `16px`, `1.5`, `-8`).
fn parse_num(value: &str, name: &str, selector: &str) -> f32 {
    let t = value.trim();
    let t = t.strip_suffix("px").unwrap_or(t).trim();
    t.parse::<f32>().unwrap_or_else(|_| {
        panic!(
            "elm-magic css!: `{selector}`의 `{name}` 값 `{value}`는 숫자가 아닙니다 (예: `16`, `16px`, `1.5`)"
        )
    })
}

/// 값에서 숫자 여러 개를 뽑는다 (`-` 부호 포함).
fn parse_nums(value: &str, name: &str, selector: &str) -> Vec<f32> {
    let mut nums: Vec<f32> = Vec::new();
    let mut negative = false;
    for word in value.split_whitespace() {
        if word == "-" {
            negative = true;
            continue;
        }
        let v = parse_num(word, name, selector);
        nums.push(if negative { -v } else { v });
        negative = false;
    }
    nums
}

/// 1·2·4개 숫자 → `Edges` 생성 코드.
fn parse_edges(value: &str, name: &str, selector: &str) -> String {
    let nums = parse_nums(value, name, selector);
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
            "elm-magic css!: `{selector}`의 `{name}`은 1·2·4개의 숫자를 받습니다 (예: `16`, `8 16`, `8 16 4 2`) — `{value}`"
        ),
    }
}

/// 그림자 값 → `Shadow` 리터럴.
///
/// `blur` / `dy blur` / `dy blur spread` / `dx dy blur spread` (4값은 CSS 순서).
fn shadow_expr(value: &str, name: &str, selector: &str) -> String {
    let n = parse_nums(value, name, selector);
    let shadow = |dx: f32, dy: f32, blur: f32, spread: f32| {
        format!(
            "::elm_magic::style::Shadow {{ dx: {dx}f32, dy: {dy}f32, blur: {blur}f32, spread: {spread}f32 }}"
        )
    };
    match n.len() {
        1 => shadow(0.0, n[0] / 2.0, n[0], 0.0),
        2 => shadow(0.0, n[0], n[1], 0.0),
        3 => shadow(0.0, n[0], n[1], n[2]),
        4 => shadow(n[0], n[1], n[2], n[3]),
        _ => panic!(
            "elm-magic css!: `{selector}`의 `{name}`은 1~4개의 숫자를 받습니다 (blur / dy blur / dy blur spread / dx dy blur spread) — `{value}`"
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
            let left = out
                .trim_end()
                .chars()
                .next_back()
                .map(word)
                .unwrap_or(false);
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
