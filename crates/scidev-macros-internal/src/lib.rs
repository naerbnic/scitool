//! A library of procedural macros used in the development of `scidev`.

mod other_fn;

/// A macro that allows the use of `?` operator in functions that return
/// `Result<T, OtherError>`. Since `OtherError` implement`std::error::Error`or,
/// we cannot have an arbitrary conversion from other error types to `OtherError`.
/// due to trait coherence rules. This macro rewrites the function to return
/// `Result<T, Box<dyn std::error::Error>>` instead, allowing the use of `?`
/// with any error type, converting the results to an `OtherError` at the call
/// site if needed.
#[proc_macro_attribute]
pub fn other_fn(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match other_fn::other_fn(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
