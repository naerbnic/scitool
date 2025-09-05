mod context;
mod invalid_data;
mod no_error;
mod other;

pub mod prelude {
    pub use super::ErrorExt as _;
    pub use super::context::ResultExt as _;
    pub use super::other::OptionExt as _;
    pub use super::other::ResultExt as _;
}

pub(crate) use invalid_data::{AnyInvalidDataError, BlockContext, InvalidDataError};
pub use no_error::NoError;
pub(crate) use other::{OtherError, bail_other, ensure_other};

pub trait ErrorExt {
    fn get_in_chain<E: std::error::Error + 'static>(&self) -> Option<&E>;
}

impl<E> ErrorExt for E
where
    E: std::error::Error + 'static,
{
    fn get_in_chain<Target: std::error::Error + 'static>(&self) -> Option<&Target> {
        let mut current: &(dyn std::error::Error + 'static) = self;
        loop {
            if let Some(target) = current.downcast_ref::<Target>() {
                return Some(target);
            }
            match current.source() {
                Some(source) => current = source,
                None => return None,
            }
        }
    }
}
