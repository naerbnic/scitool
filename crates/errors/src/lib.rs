#![doc = include_str!("lib_docs.md")]

mod binders;
mod define_error;
mod diag;
mod ext;
mod finding;
mod fmt_helpers;
mod frame;
mod reportable;

pub use binders::{
    ContextBinder, IntoCause, OptionRaiseBinder, ResultContextBinder, ResultRaiseBinder,
};

pub use diag::{AnyDiag, Diag, DiagLike, Kind, MaybeDiag};
pub use ext::{DiagStdError, OptionExt, RaisedKind, RaisedMaybe, RaisedMessage, Raiser, ResultExt};
pub use frame::{ContextView, ErrorView, TypedErrorView};
pub use reportable::Reportable;

pub mod prelude {
    // This should only include nameless trait imports, to not pollute the
    // namespace on `use scidev_errors::prelude::*`
    pub use crate::{DiagLike as _, OptionExt as _, ResultExt as _};
}
