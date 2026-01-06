//! Test helpers for tests in this crate.
#![cfg(test)]

use std::collections::BTreeMap;

pub(crate) fn make_map(
    items: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
) -> BTreeMap<String, String> {
    items
        .into_iter()
        .map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
        .collect()
}

macro_rules! assert_matches {
    ($expr: expr, $pat:pat) => {{
        let expr = $expr;
        assert!(
            matches!(expr, $pat),
            "Match failed:\n\texpected: {:?}\n\tactual: {:?}",
            stringify!($pat),
            expr
        );
    }};
}

macro_rules! from_json {
    ($($json:tt)*) => {
        serde_json::from_value(serde_json::json!( $($json)* )).unwrap()
    };
}

pub(crate) use assert_matches;
pub(crate) use from_json;
