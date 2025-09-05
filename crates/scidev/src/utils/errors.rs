mod context;
mod invalid_data;
mod other;

pub mod prelude {
    pub use super::ErrorExt as _;
    pub use super::context::ResultExt as _;
    pub use super::other::OptionExt as _;
    pub use super::other::ResultExt as _;
}

pub(crate) use invalid_data::{AnyInvalidDataError, BlockContext, InvalidDataError};
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

#[derive(Debug, Copy, Clone)]
pub enum NoError {}

impl std::fmt::Display for NoError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {}
    }
}
impl std::error::Error for NoError {}

pub trait NoErrorResultExt<T> {
    fn into_ok(self) -> T;
}

impl<T> NoErrorResultExt<T> for Result<T, NoError> {
    fn into_ok(self) -> T {
        match self {
            Ok(value) => value,
        }
    }
}
