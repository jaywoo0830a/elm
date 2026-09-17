//! Procedural macros for elm-magic.
//!
//! - `view!` — turns a function with default-valued params into a `Component`.
//! - `ui!` — JSX-like element builder expression.
//! - `css!` — style registry registration.

mod css;
mod jsx;
mod store;
mod view;

use proc_macro::TokenStream;

/// Define a component: parameters are state slots, the body is the UI.
///
/// Takes the whole function as raw tokens (parameters may have
/// non-Rust default values like `n = 0`).
///
/// ```ignore
/// elm_magic::view! {
///     fn Counter(n = 0) {
///         <Row>
///             <Button on_click={n += 1}>"+"</Button>
///             "Count: {n}"
///         </Row>
///     }
/// }
/// ```
#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    view::expand(input)
}

/// Define global state (사양서 4.2): every field becomes a name-keyed arena slot
/// shared by all components.
///
/// ```ignore
/// #[store]
/// struct App { user: Option<User>, theme: Theme = Theme::Dark }
///
/// elm_magic::view! {
///     fn Header() {
///         "theme: "{app.theme}
///         <Button on_click={app.theme = app.theme.clone().toggle()}>"theme"</Button>
///     }
/// }
/// ```
///
/// `#[store]`는 사용하는 `view!`보다 **위에** 선언한다 (proc-macro 레지스트리).
#[proc_macro_attribute]
pub fn store(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _ = attr;
    store::expand_attribute(item)
}

/// `store! { App { user: Option<User>, theme: Theme = Theme::Dark } }` —
/// 함수형 형태 (사양서 10.4). 필드 기본값(`field: T = expr`)을 쓸 수 있다는 점이
/// `#[store]`와 다르다 (속성 매크로 입력은 rustc가 구조체로 파싱하므로
/// 필드 기본값 문법은 아직 불안정).
#[proc_macro]
pub fn store_fn(input: TokenStream) -> TokenStream {
    store::expand_fn(input)
}

/// JSX-like element builder.
///
/// ```ignore
/// let el = ui! { <Col class="app"><Text>"hi"</Text></Col> };
/// ```
#[proc_macro]
pub fn ui(input: TokenStream) -> TokenStream {
    let toks: Vec<proc_macro::TokenTree> = input.into_iter().collect();
    let states = std::collections::HashSet::new();
    let slots = std::cell::Cell::new(0usize);
    let callbacks = std::collections::HashSet::new();
    let stores = std::collections::HashMap::new();
    let env = jsx::Env::empty(&states, &slots, &callbacks, &stores);
    jsx::transform_top(&toks, &env)
}

/// Define styles (사양서 6.1 — Tailwind-like CSS).
///
/// ```ignore
/// elm_magic::css! {
///     .card { gap: 8; padding: 16; bg: surface; }
///     button { bg: primary; }
/// }
/// ```
#[proc_macro]
pub fn css(input: TokenStream) -> TokenStream {
    css::expand(input)
}
