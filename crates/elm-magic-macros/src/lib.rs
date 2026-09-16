//! Procedural macros for elm-magic.
//!
//! - `view!` — turns a function with default-valued params into a `Component`.
//! - `ui!` — JSX-like element builder expression.
//! - `css!` — style registry registration.

mod css;
mod jsx;
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

/// JSX-like element builder.
///
/// ```ignore
/// let el = ui! { <Col class="app"><Text>"hi"</Text></Col> };
/// ```
#[proc_macro]
pub fn ui(input: TokenStream) -> TokenStream {
    let toks: Vec<proc_macro::TokenTree> = input.into_iter().collect();
    let env = jsx::Env {
        states: &std::collections::HashSet::new(),
        slots: &std::cell::Cell::new(0),
    };
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
