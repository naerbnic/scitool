#![doc = include_str!("lib_docs.md")]

mod binders;
mod define_error;
mod diag;
mod ext;
mod finding;
mod fmt_helpers;
mod frame;
mod raiser;
mod reportable;
mod sealed;

pub mod out;

pub use binders::{ContextBinder, IntoCause, RaiseBinder};

pub use diag::{AnyDiag, Diag, DiagLike, Kind, MaybeDiag};
pub use ext::{AnyDiagStdError, DiagStdError, OptionExt, ResultExt};
pub use frame::{ContextView, ErrorView, TypedErrorView};
pub use raiser::{RaisedKind, RaisedMaybe, RaisedMessage, Raiser};
pub use reportable::Reportable;

/// A "prelude" module for users of the `scidev-errors` crate.
///
/// This module only imports traits that are typically used by this crate, but
/// does not import their names. Users are intended to import this as:
///
/// ```
/// use scidev_errors::prelude::*;
/// ```
pub mod prelude {
    // This should only include nameless trait imports, to not pollute the
    // namespace on `use scidev_errors::prelude::*`
    pub use crate::{DiagLike as _, OptionExt as _, ResultExt as _};
}
