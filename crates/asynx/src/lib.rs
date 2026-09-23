//! Internal private crate that wraps dependencies from other futures ecosystem
//! crates. This should contain minimal logic, mostly republishing existing
//! types.

pub mod executor;
pub mod flavor;

pub mod deps {
    pub use tokio;
}

/// Most of the io traits here are from Tokio, as that seems to be the agreed
/// upon consistent version. These traits do not depend directly on the Tokio
/// runtime, so this is still part of our generic base.
pub mod io {
    pub use tokio::io::{
        AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt,
        AsyncWrite, AsyncWriteExt, BufReader, BufStream, BufWriter, Chain, DuplexStream, Empty,
        Error, ErrorKind, Join, Lines, ReadBuf, ReadHalf, Repeat, Result, SeekFrom, SimplexStream,
        Sink, Split, Take, WriteHalf,
    };

    // Functions
    pub use tokio::io::{
        copy, copy_bidirectional, copy_bidirectional_with_sizes, copy_buf, duplex, empty, join,
        repeat, simplex, sink, split,
    };
}

pub mod channel {
    pub mod mpsc {
        pub use futures_channel::mpsc::{
            Receiver, Recv, RecvError, SendError, Sender, TryRecvError, TrySendError,
            UnboundedReceiver, UnboundedSender, channel, unbounded,
        };
    }

    pub mod oneshot {
        pub use futures_channel::oneshot::{Canceled, Cancellation, Receiver, Sender, channel};
    }
}

pub mod future {
    pub use futures_util::future::{
        AbortHandle, AbortRegistration, Abortable, Aborted, AlwaysReady, AndThen, BoxFuture,
        CatchUnwind, Either, ErrInto, Flatten, FlattenSink, FlattenStream, Fuse, FusedFuture,
        Future, FutureExt, FutureObj, Inspect, InspectErr, InspectOk, IntoFuture, IntoStream, Join,
        Join3, Join4, Join5, JoinAll, Lazy, LocalBoxFuture, LocalFutureObj, Map, MapErr, MapInto,
        MapOk, MapOkOrElse, MaybeDone, NeverError, OkInto, OptionFuture, OrElse, Pending, PollFn,
        PollImmediate, Ready, Remote, RemoteHandle, Select, SelectAll, SelectOk, Shared, Then,
        TryFlatten, TryFlattenStream, TryFuture, TryFutureExt, TryJoin, TryJoin3, TryJoin4,
        TryJoin5, TryJoinAll, TryMaybeDone, TrySelect, UnitError, UnsafeFutureObj, UnwrapOrElse,
        WeakShared, abortable, always_ready, err, join, join_all, join3, join4, join5, lazy,
        maybe_done, ok, pending, poll_fn, poll_immediate, ready, select, select_all, select_ok,
        try_join, try_join_all, try_join3, try_join4, try_join5, try_maybe_done, try_select,
    };
}

pub mod stream {
    pub use futures_util::stream::{
        AbortHandle, AbortRegistration, Abortable, Aborted, All, AndThen, Any, BoxStream,
        BufferUnordered, Buffered, CatchUnwind, Chain, Chunks, Collect, Concat, Count, Cycle,
        Empty, Enumerate, ErrInto, Filter, FilterMap, FlatMap, FlatMapUnordered, Flatten,
        FlattenUnordered, Fold, ForEach, ForEachConcurrent, Forward, Fuse, FusedStream,
        FuturesOrdered, FuturesUnordered, Inspect, InspectErr, InspectOk, IntoStream, Iter,
        LocalBoxStream, Map, MapErr, MapOk, Next, NextIf, NextIfEq, Once, OrElse, Peek, PeekMut,
        Peekable, Pending, PollFn, PollImmediate, PollNext, ReadyChunks, Repeat, RepeatWith,
        ReuniteError, Scan, Select, SelectAll, SelectNextSome, SelectWithStrategy, Skip, SkipWhile,
        SplitSink, SplitStream, Stream, StreamExt, StreamFuture, Take, TakeUntil, TakeWhile, Then,
        TryAll, TryAny, TryBufferUnordered, TryBuffered, TryChunks, TryChunksError, TryCollect,
        TryConcat, TryFilter, TryFilterMap, TryFlatten, TryFlattenUnordered, TryFold, TryForEach,
        TryForEachConcurrent, TryNext, TryReadyChunks, TryReadyChunksError, TrySkipWhile,
        TryStream, TryStreamExt, TryTakeWhile, TryUnfold, Unfold, Unzip, Zip, abortable, empty,
        iter, once, pending, poll_fn, poll_immediate, repeat, repeat_with, select,
        select_with_strategy, try_unfold, unfold,
    };

    pub mod futures_unordered {
        pub use futures_util::stream::futures_unordered::{
            FuturesUnordered, IntoIter, Iter, IterMut, IterPinMut, IterPinRef,
        };
    }

    pub mod select_all {
        pub use futures_util::stream::select_all::{
            IntoIter, Iter, IterMut, SelectAll, select_all,
        };
    }
}

pub mod prelude {
    pub use futures_util::{
        future::{FutureExt as _, TryFutureExt as _},
        sink::SinkExt as _,
        stream::{StreamExt as _, TryStreamExt as _},
    };
    pub use tokio::io::{
        AsyncBufReadExt as _, AsyncReadExt as _, AsyncSeekExt as _, AsyncWriteExt as _,
    };
}

pub use futures_util::{join, try_join};
