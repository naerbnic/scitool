//! A crate that contains pub types that are intended to be used as sources of
//! sealed types and traits.
#![doc(hidden)]

use crate::RaisedMessage;

pub trait Sealed {}

pub struct SealedToken;

pub trait DiagLikePriv {
    fn add_context_message(&mut self, msg: RaisedMessage);
}
