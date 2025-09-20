//! Provides an arc type that moves ownership, rather than just the reference.
//!
//! This is useful to be able to pass a 'static object into a closure that
//! doesn't require explicit locking, but also needs the value back after the
//! closure is run.
#![allow(unsafe_code, reason = "Unsafe usage is specified here")]

use std::{cell::UnsafeCell, sync::Arc};

struct Inner<T: ?Sized> {
    value: UnsafeCell<T>,
}

pub struct MutBorrowedArc<T: ?Sized>(Arc<Inner<T>>);

impl<T> MutBorrowedArc<T> {
    /// Create an instance of `MutBorrowedArc` that is not lent out.
    ///
    /// This behaves identically, but does not require a `LentArc` to be created.
    pub fn new(value: T) -> Self {
        let arc = std::sync::Arc::new(Inner {
            value: UnsafeCell::new(value),
        });
        Self(arc)
    }
}

impl<T: ?Sized> std::ops::Deref for MutBorrowedArc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY: MutBorrowedArc provides ownership semantics, so there can
        // never be multiple mutable references to the same value.
        unsafe { &*self.0.value.get() }
    }
}

impl<T: ?Sized> std::ops::DerefMut for MutBorrowedArc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: MutBorrowedArc provides ownership semantics, so there can
        // never be multiple mutable references to the same value.
        unsafe { &mut *self.0.value.get() }
    }
}

unsafe impl<T: ?Sized + Send> Send for MutBorrowedArc<T> {}
unsafe impl<T: ?Sized + Sync> Sync for MutBorrowedArc<T> {}

/// A value that indicates the contained value is lent out. It cannot be used
/// again until it is returned.
pub struct LentArc<T: ?Sized>(Arc<Inner<T>>);

impl<T> LentArc<T> {
    pub fn try_take_back(self) -> Result<T, Self>
    where
        T: Clone,
    {
        let owned_inner = Arc::try_unwrap(self.0).map_err(Self)?;
        Ok(owned_inner.value.into_inner())
    }

    #[must_use]
    pub fn take_back(self) -> T {
        let Ok(value) = Arc::try_unwrap(self.0) else {
            panic!("Cannot take back value; it is still lent out");
        };
        value.value.into_inner()
    }
}

pub fn loan_arc<T>(value: T) -> (MutBorrowedArc<T>, LentArc<T>) {
    let arc = std::sync::Arc::new(Inner {
        value: UnsafeCell::new(value),
    });
    (MutBorrowedArc(arc.clone()), LentArc(arc))
}

// Foreign trait implementations.

impl<T> tokio::io::AsyncRead for MutBorrowedArc<T>
where
    T: tokio::io::AsyncRead + Unpin + ?Sized,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this: &mut T = self.get_mut();
        std::pin::Pin::new(this).poll_read(cx, buf)
    }
}

impl<T> tokio::io::AsyncWrite for MutBorrowedArc<T>
where
    T: tokio::io::AsyncWrite + Unpin + ?Sized,
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this: &mut T = self.get_mut();
        std::pin::Pin::new(this).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this: &mut T = self.get_mut();
        std::pin::Pin::new(this).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this: &mut T = self.get_mut();
        std::pin::Pin::new(this).poll_shutdown(cx)
    }
}
