use crate::executor::{Executor, ExecutorImpl};

pub(crate) struct SimpleExecutor {}

impl ExecutorImpl for SimpleExecutor {
    fn schedule_sync<F>(&self, func: F)
    where
        F: FnOnce() + Send + 'static,
    {
        std::thread::spawn(func);
    }

    fn schedule_async<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.schedule_sync(Box::new(move || futures_executor::block_on(fut)));
    }

    fn block_in_place<F>(&self, func: F)
    where
        F: FnOnce(),
    {
        // We have no runtime. Assume that we can just run it directly.
        func();
    }
}

pub(crate) fn make_simple() -> Executor {
    Executor::new(SimpleExecutor {})
}
