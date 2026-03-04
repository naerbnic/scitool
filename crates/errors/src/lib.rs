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

pub use binders::{Bind, ContextBind, ContextBinder, IntoCause, RaiseBinder};

pub use diag::{AnyDiag, Diag, DiagLike, Kind, MaybeDiag};
pub use ext::{DiagStdError, OptionExt, ResultExt};
pub use frame::{ContextView, ErrorView, TypedErrorView};
pub use raiser::{RaisedKind, RaisedMaybe, RaisedMessage, Raiser};
pub use reportable::Reportable;

pub mod prelude {
    // This should only include nameless trait imports, to not pollute the
    // namespace on `use scidev_errors::prelude::*`
    pub use crate::{DiagLike as _, OptionExt as _, ResultExt as _};
}
