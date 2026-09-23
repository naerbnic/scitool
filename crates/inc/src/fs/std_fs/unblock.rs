//! Helpers for unblocking Synchronous I/O onto a synchronous executor.
//!
//! This is based on the `unblock` crate, but using our own executor
//! implementation, rather than using the one from that crate.

use std::io;

use asynx::io as async_io;

use crate::promise::Promise;

type Buffer = Vec<u8>;

pub(super) struct Unblock<T> {
    state: State<T>,
    cap: usize,
}

enum State<T> {
    Idle {
        io_handle: T,
        buffer: Buffer,
    },
    Reading {
        result: Promise<(T, io::Result<u64>, Buffer)>,
    },
    Writing {
        result: Promise<(T, io::Result<u64>, Buffer)>,
    },
    Seeking {
        result: Promise<(T, io::Result<u64>)>,
    },
}

impl<T> async_io::AsyncRead for Unblock<T>
where
    T: io::Read + Unpin + Send + 'static,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut async_io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        // let exec = crate::executor::Handle::current();
        // match &mut self.as_mut().state {
        //     State::Reading { result } => ready!(result.poll),
        //     State::Writing { result } => todo!(),
        //     State::Seeking { result } => todo!(),
        //     State::Idle { io_handle, buffer } => todo!(),
        // }
        todo!()
    }
}
