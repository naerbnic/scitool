//! A crate that contains pub types that are intended to be used as sources of
//! sealed types and traits.
#![doc(hidden)]

use crate::RaisedMessage;

/// A supertrait for traits that should not be implemented by clients.
pub trait Sealed {}

/// A token value that cannot be created by clients. When used as a parameter,
/// ensures a function or trait method cannot be called by clients.
pub struct SealedToken;

pub trait DiagLikePriv {
    fn add_context_message(&mut self, msg: RaisedMessage);
}
