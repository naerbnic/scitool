mod invalid_data;
mod no_error;
mod other;

pub mod prelude {
    pub(crate) use super::{OtherOptionExt as _, OtherResultExt as _};
}

pub(crate) use self::other::{OptionExt as OtherOptionExt, ResultExt as OtherResultExt};
pub(crate) use invalid_data::{AnyInvalidDataError, BlockContext, InvalidDataError};
pub use no_error::NoError;
pub(crate) use other::{
    BoxError, CastChain, DynError, ErrWrapper, OtherError, bail_other, ensure_other,
};
