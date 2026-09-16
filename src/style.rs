//! Minimal style registry (사양서 6.1) — classes interned as selector → props.
//!
//! `css!` registers selectors at startup; renderers compare interned ids.
//! Headless tests only verify registration and lookup.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A registered selector with its property list.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleProps {
    selector: String,
    props: Vec<(String, String)>,
}

impl StyleProps {
    pub fn selector(&self) -> &str {
        &self.selector
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.props
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

fn registry() -> &'static Mutex<HashMap<String, StyleProps>> {
    static REG: OnceLock<Mutex<HashMap<String, StyleProps>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register selectors (called by `css!`). Idempotent per selector.
pub fn register(entries: Vec<(&str, Vec<(&str, &str)>)>) {
    let mut reg = registry().lock().unwrap();
    for (selector, props) in entries {
        let props = props
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        reg.entry(selector.to_string())
            .or_insert(StyleProps { selector: selector.to_string(), props });
    }
}

/// Look up a registered selector (e.g. `".card"` or `"button"`).
pub fn lookup(selector: &str) -> Option<StyleProps> {
    registry().lock().unwrap().get(selector).cloned()
}
