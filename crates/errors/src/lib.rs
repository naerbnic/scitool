#![doc = include_str!("lib_docs.md")]

mod binders;
mod causes;
mod define_error;
mod diag;
mod dyn_err_conversion;
mod ext;
mod finding;
mod fmt_helpers;
mod frame;
mod helpers;
mod locations;
mod raiser;
mod reportable;
mod sealed;

pub mod out;

pub use binders::{ContextBinder, RaiseBinder};
pub use causes::IntoCause;

pub use diag::{AnyDiag, Diag, DiagLike, Kind, MaybeDiag};
pub use ext::{OptionExt, ResultExt};
pub use frame::{ContextView, ErrorView, TypedErrorView};
pub use raiser::{RaisedKind, RaisedMaybe, RaisedMessage, Raiser};
pub use reportable::Reportable;

pub use helpers::{AnyDiagErrorCatcher, ErrorContextBinder, in_err_context};

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

/// A module for exporting symbols that need to be available for use in macros,
/// but should not be used directly by clients.
#[doc(hidden)]
pub mod __private {
    pub use crate::dyn_err_conversion::AnyWrapper;
}
