use asynx::deps::tokio::{runtime::Handle, task::block_in_place};

use crate::executor::ExecutorImpl;

pub(super) struct TokioExecutor {
    handle: Handle,
}

impl TokioExecutor {
    pub(super) fn new(handle: &Handle) -> Self {
        Self {
            handle: handle.clone(),
        }
    }
}

impl ExecutorImpl for TokioExecutor {
    fn schedule_sync<F>(&self, func: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let _join_handle = self.handle.spawn_blocking(func);
    }

    fn schedule_async<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let _join_handle = self.handle.spawn(fut);
    }

    fn block_in_place<F>(&self, func: F)
    where
        F: FnOnce(),
    {
        if Handle::try_current().is_ok() {
            block_in_place(func);
        } else {
            // We have no runtime, meaning this can be called on another
            // synchronous thread. Assume that we can just run it directly.
            func();
        }
    }
}
