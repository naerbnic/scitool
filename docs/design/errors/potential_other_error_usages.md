# Potential Places to Adopt `OtherError`

This document lists locations in the codebase that currently use other error handling mechanisms (like `anyhow` or `io::Error` for logic errors) but are good candidates for migration to `OtherError` (using `ensure_other!`, `bail_other!`, etc.).

## `crates/scidev`

These locations currently use `std::io::Error` with `ErrorKind::InvalidData` or similar for what appear to be logical or validation errors.

* **`src/resources/file.rs`**
  * **Line 257**: `Err(io::Error::new(io::ErrorKind::InvalidData, format!("Duplicate resource ID: {id:?}")))`
    * **Suggestion**: Use `bail_other!("Duplicate resource ID: {:?}", id)`

* **`src/utils/block/core.rs`**
  * **Line 405**: `Err(io::Error::new(io::ErrorKind::UnexpectedEof, ...))`
    * **Suggestion**: If `FromBlock` trait signature allows, change return type to `Result<..., OtherError>` and use `bail_other!`.

* **`src/utils/block/core/seq_impl.rs`**
  * **Line 64**: `Err(io::Error::new(io::ErrorKind::InvalidData, "Sequence block read out of bounds"))`
  * **Line 97**: `Err(io::Error::new(io::ErrorKind::InvalidData, "Sequence block read out of bounds"))`
    * **Suggestion**: Use `bail_other!`.

* **`src/utils/block/core/empty_impl.rs`**
  * **Line 14, 25**: `Err(io::Error::new(io::ErrorKind::InvalidData, "Empty block read out of bounds"))`
    * **Suggestion**: Use `bail_other!`.

## `crates/sciproj`

This crate currently uses `anyhow` extensively. Since it depends on `scidev`, it can adopt `scidev::utils::errors::OtherError` for internal errors to align with the workspace "house rules".

* **`src/resources.rs`**
  * **Line 95**: `anyhow::ensure!(clip_path.is_relative(), ...)`
    * **Suggestion**: `ensure_other!(clip_path.is_relative(), ...)`
  * **Line 141**: `anyhow::ensure!(!scan.has_duplicates(), ...)`
    * **Suggestion**: `ensure_other!(!scan.has_duplicates(), ...)`
  * **Line 180**: `anyhow::ensure!(clip.start_us.is_none_or(|off| off == 0))`
    * **Suggestion**: `ensure_other!(...)`
  * **Line 194**: `Err(anyhow::anyhow!("Audio clip ..."))`
    * **Suggestion**: `bail_other!("Audio clip ...")`

* **`src/file.rs`**
  * **Line 39**: `.map_err(|_| anyhow::anyhow!("Failed to strip prefix"))`
    * **Suggestion**: `.map_err(|_| OtherError::from_msg("Failed to strip prefix"))`

## `crates/scitool-cli`

This is a CLI crate, so `anyhow` is acceptable for the `main` function, but internal logic could benefit from typed errors or `OtherError`.

* **`src/commands/messages.rs`**
  * **Line 5**: `pub fn print_talkers(...) -> anyhow::Result<()>`
    * **Suggestion**: Consider if this should return `Result<(), OtherError>` if it's library-like code.
