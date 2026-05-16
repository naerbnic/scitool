//! Contains internal dependencies, to make it easier to read our dependencies
//! in implementations.
#![allow(
    unused_imports,
    reason = "Module is for general imports. Will be reduced later"
)]

pub(crate) mod futures {
    pub(crate) mod prelude {
        pub(crate) use super::io::{
            AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt,
        };
        pub(crate) use super::stream::{Stream, StreamExt, TryStream, TryStreamExt};
        pub(crate) use super::{FutureExt, TryFutureExt};
    }

    pub(crate) mod stream {
        pub(crate) use futures_util::stream::FuturesUnordered;
        pub(crate) use futures_util::stream::{Stream, StreamExt, TryStream, TryStreamExt, iter};
    }

    pub(crate) use tokio::io;
    pub(crate) use tokio::sync;

    pub(crate) use futures_core::future::BoxFuture;
    pub(crate) use futures_util::{FutureExt, TryFutureExt, join, ready, try_join};
}
