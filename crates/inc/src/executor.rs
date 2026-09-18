mod simple;
use std::pin::Pin;

pub(crate) struct Executor {
    exec_impl: Box<dyn ExecutorImpl>,
}

impl Executor {
    pub(crate) fn new<I: ExecutorImpl>(exec_impl: I) -> Self {
        Self {
            exec_impl: Box::new(exec_impl),
        }
    }

    #[expect(dead_code)]
    pub(crate) fn schedule<F>(&self, func: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.exec_impl.schedule_impl(Box::new(func));
    }

    pub(crate) fn schedule_async<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.exec_impl.schedule_async_impl(Box::pin(fut));
    }
}

impl Executor {
    pub(crate) fn new_simple() -> Self {
        simple::make_simple()
    }
}

pub(crate) trait ExecutorImpl: Send + Sync + 'static {
    fn schedule_impl(&self, func: Box<dyn FnOnce() + Send + 'static>);
    fn schedule_async_impl(&self, fut: Pin<Box<dyn Future<Output = ()> + Send + 'static>>);
    #[expect(dead_code)]
    fn shutdown(self: Box<Self>);
}
