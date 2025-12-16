mod cast;
mod invalid_data;
mod no_error;
mod opaque;
mod other;
mod unpack;

pub mod prelude {
    pub(crate) use super::{OtherOptionExt as _, OtherResultExt as _};
}

pub(crate) type DynError = dyn std::error::Error + Send + Sync + 'static;
pub(crate) type BoxError = Box<DynError>;

pub(crate) use scidev_macros_internal::other_fn;

pub(crate) use self::other::{OptionExt as OtherOptionExt, ResultExt as OtherResultExt};
pub(crate) use invalid_data::BlockContext;
pub use invalid_data::InvalidDataError;
pub use no_error::NoError;
pub use opaque::OpaqueError;
pub(crate) use other::{OtherError, bail_other, ensure_other};

pub(crate) use cast::{Builder as ErrorCastBuilder, ErrorCast, ErrorCastable, impl_error_castable};
pub(crate) use unpack::{ErrWrapper, UnpackableError, resolve_error};
