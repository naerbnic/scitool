# `scidev-errors`

An opinionated error crate for handling error types with the actionable
philosophy.

```no_run
use scidev_errors::{Diag, AnyDiag, MaybeDiag, Kind, ensure, bail, prelude::*};
use std::fs;

// Define an error kind that you want to work with.
//
// Note: This does not have to be an enum, and it does not need to be
// Displayable.
#[derive(Debug)]
pub enum ConfigError {
    MissingConfigFile,
    MalformedSyntax,
}

// Implement the marker trait `Kind` for your error type.
//
// This makes it clear this is intended to act as an actionable error.
impl Kind for ConfigError {}

// You can propagate existing errors by raising your own.
fn read_config(path: &str) -> Result<String, Diag<ConfigError>> {
    // We expect that the config may be missing, so we convert the 
    // io::ErrorKind::NotFound error into a labelled diag.
    fs::read_to_string(path).map_raise(|err, r| {
        // You can use `r` here (of type `Raiser`) to generate the intended
        // kind of error.
        match err.kind() {
            std::io::ErrorKind::NotFound => r.kind_msg(
                ConfigError::MissingConfigFile, "File not found"),
            _ => r.kind_msg(
                ConfigError::MalformedSyntax, "Could not read file data"),
        }
    })
}

// 3. A higher-level function that adds context to the error as it bubbles up
fn load_app() -> Result<(), Diag<ConfigError>> {
    let _config = read_config("app.toml")
        .with_context()
        .msg("Failed to initialize the application during startup")?;
    
    Ok(())
}

fn main() {
    if let Err(diag) = load_app() {
        // Prints the structured error, preserving the original io::Error cause and context!
        println!("{:#}", diag);
    }
}
```

## Philosophy

The actionable error philosophy is that the only discernable error types
given to a caller should be those which are in some way actionable, meaning
there is some way for the caller code to handle the error in response to the
error value. If the only thing a caller could reasonably do is log and/or
display the error to the user, and then either exit or perform a different
fallback operation, then there is no reason to provide an error type that
can distinguish between different causes.

For example, errors like and `io::Error` with an `AlreadyExists` for creating
a file is often actionable, either letting the program read the file they
were intending to create, but other kinds like `InvalidData` often don't
have any particular way the client can really fix it.

In addition, it's often more important to be able to add human-readable
context to an error, either for debuggability, or to provide better
information for the error result. Errors should allow us to add arbitrary
context without modifying the core error type.

This is inspired by the exn crate, but with additional wiring for more
complex error handling, and to create public error types that do not
depend on the [`Diag`] and [`AnyDiag`] types, and fit with the existing
[`std::error::Error`] ecosystem.

## Actionability

The `Actionability` of a Diag value indicates whether or not the contained
values are intended to be consumed by the caller of a function, or if it is
only a collected error trace. Actionability is maintained strictly, both
through types and dynamically. An error value is only obtainable from a
Diag value if the value was originally actionable. If you have a value
`err` that you want to treat as actionable, you must use one of the
following to create a value:

- [`Diag::new()`]
- [`Diag::with_causes()`]
- [`bail!`] with the value as the first parameter
- [`ensure!`] with the value as the second (first non-boolean) parameter.
- [`ResultExt::raise()`]
- [`ResultExt::map_raise()`]

If a Diag value is coerced to [`AnyDiag`], through
[`std::convert::Into::into`], any value that was contained will be
considered unactionable, and the value can't be reacquired through the
non-view API.

[`MaybeDiag<E>`] allows both [`Diag<E>`] and [`AnyDiag`] to be coerced to
it. It keeps track of which source it came from, so if you try to round trip
an error through [`Diag<E>`] -> [`AnyDiag`] -> [`MaybeDiag<E>`],
[`MaybeDiag::opt_kind()`] will still return `None`.

## Using Context

Context is intended to be a way of adding additional information to an
existing error, without changing its actionable contents. This should be
used judiciously, whenver the calling context will be useful.

## Examples
