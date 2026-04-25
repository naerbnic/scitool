//! We need to be able to convert from [`std::error::Error`] dynamically to an
//! `AnyDiag` without making a type implement a trait. We use the dynamic parts
//! of the [`std::error::Error`] API to implement this.
//!
//! The version that is available in stable is to use the `source()`, which
//! returns a `dyn Error`, to return a type with a concrete base that we can
//! test for directly. This is not completely ideal, as it is frequently also
//! used to track the entire source path, but given that the contents are
//! intended to be used as a black box, this is likely fine for now.
//!
//! If stabilized, the provider interface can likely work as a better
//! way of obtaining this, separately from the source method itself.

#![allow(unsafe_code)]

use std::{
    any::Any,
    cell::UnsafeCell,
    fmt::{Debug, Display},
};

use crate::{AnyDiag, DiagLike};

trait ReportableAny: Any + Send + Sync {
    #[must_use]
    unsafe fn take_any(&self) -> AnyDiag;
    fn display(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}

struct DiagContents<T>(UnsafeCell<Option<T>>);

// SAFETY: The UnsafeCell is intended to be available as the base value, until
// taken.
#[allow(unsafe_code)]
unsafe impl<T> Send for DiagContents<T> where T: Send {}

// SAFETY: The UnsafeCell is intended to be available as the base value, until
// taken.
#[allow(unsafe_code)]
unsafe impl<T> Sync for DiagContents<T> where T: Sync {}

#[allow(unsafe_code)]
impl<T> DiagContents<T>
where
    T: DiagLike,
{
    #[must_use]
    fn new(diag: T) -> Self {
        Self(UnsafeCell::new(Some(diag)))
    }

    #[must_use]
    fn get(&self) -> &T {
        unsafe { self.0.get().as_ref() }.unwrap().as_ref().unwrap()
    }
}

impl<T> ReportableAny for DiagContents<T>
where
    T: DiagLike,
{
    /// Take the value from the contents of the unsafe cell.
    ///
    /// This is intended to be used once, and getting the contents will panic
    /// if the contents are accessed twice.
    ///
    /// # SAFETY
    ///
    /// It must be sure that there are no other synchronous shared references
    /// to this object.
    unsafe fn take_any(&self) -> AnyDiag {
        unsafe { self.0.get().as_mut() }
            .unwrap()
            .take()
            .unwrap()
            .into_any_diag()
    }
    fn display(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.get(), f)
    }

    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.get(), f)
    }
}

/// A [`std::error::Error`] type that can be used to extract a contained [`AnyDiag`] or [`Diag`].
///
/// If another error type returns a reference to this via `source()`, then
/// the error can be converted to the contained [`AnyDiag`].
#[repr(transparent)]
pub struct AnyWrapper(Box<dyn ReportableAny>);

#[allow(unsafe_code)]
impl AnyWrapper {
    pub fn new<T>(diag: T) -> Self
    where
        T: DiagLike,
    {
        let dyn_box: Box<dyn ReportableAny> = Box::new(DiagContents::new(diag));
        Self(dyn_box)
    }
    /// Take the value from the contents of the unsafe cell, if it is a wrapped
    /// version of `AnyContents<T>` of the given type.
    ///
    /// If `Some` is returned, then the contents have been taken, and further
    /// operations other than `Drop` are likely to panic (but are not unsafe).
    ///
    /// # SAFETY
    ///
    /// It must be sure that there are no other synchronous shared references
    /// to this object, likely by having the local thread have full ownership
    /// of the object.
    #[must_use]
    pub(crate) unsafe fn take_any(&self) -> AnyDiag {
        unsafe { self.0.take_any() }
    }

    #[must_use]
    pub fn downcast_get<T>(&self) -> &T
    where
        T: DiagLike,
    {
        let any_contents: &(dyn Any + Send + Sync) = &*self.0;
        any_contents
            .downcast_ref::<DiagContents<T>>()
            .expect("Must be of defined type")
            .get()
    }
}

impl Debug for AnyWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.debug(f)
    }
}

impl Display for AnyWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.display(f)
    }
}

impl std::error::Error for AnyWrapper {}

#[allow(unsafe_code)]
pub(crate) fn try_convert_to_any_diag<E: std::error::Error + Send + Sync + 'static>(
    err: E,
) -> Result<AnyDiag, E> {
    let Some(source) = err.source() else {
        return Err(err);
    };
    let Some(any_wrap) = source.downcast_ref::<AnyWrapper>() else {
        return Err(err);
    };

    // SAFETY: We own `err`, so we know that there are no other synchronous
    // references to the source.
    Ok(unsafe { any_wrap.take_any() })
}

pub(crate) fn try_cast_to_any_diag_ref<'a>(
    err: &'a (dyn std::error::Error + 'static),
) -> Option<&'a AnyDiag> {
    err.source()?
        .downcast_ref::<AnyWrapper>()
        .map(AnyWrapper::downcast_get)
}
