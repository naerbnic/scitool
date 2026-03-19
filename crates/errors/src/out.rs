//! Types used to represent outputs from higher-kinded types.

use crate::sealed::Sealed;

/// A trait for marker types that indicate an output type that can vary with one
/// variable.
pub trait Out: Sealed {
    type Ty<T>;
}

/// Result outputs where the Ok type is bound to T, and the error varies.
#[derive(Default)]
pub struct Result<T> {
    _phantom: std::marker::PhantomData<*const T>,
}

impl<T> Out for Result<T> {
    type Ty<U> = std::result::Result<T, U>;
}

impl<T> Sealed for Result<T> {}

/// Outputs which return the varying type directly.
pub struct Value;

impl Out for Value {
    type Ty<T> = T;
}

impl Sealed for Value {}
