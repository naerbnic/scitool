use std::{sync::Mutex, thread::JoinHandle};

use crate::executor::{Executor, ExecutorImpl};

pub(crate) struct SimpleExecutor {
    pending_threads: Mutex<Vec<JoinHandle<()>>>,
}

impl ExecutorImpl for SimpleExecutor {
    fn schedule_impl(&self, func: Box<dyn FnOnce() + Send + 'static>) {
        let mut pending = self.pending_threads.lock().unwrap();
        let handle = std::thread::spawn(func);
        pending.push(handle);
    }

    fn schedule_async_impl(
        &self,
        fut: std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) {
        self.schedule_impl(Box::new(move || futures_executor::block_on(fut)));
    }

    fn shutdown(self: Box<Self>) {
        let SimpleExecutor { pending_threads } = *self;
        let pending = pending_threads.into_inner().unwrap();
        for handle in pending {
            handle.join().expect("Failed to join thread");
        }
    }
}

pub(crate) fn make_simple() -> Executor {
    Executor::new(SimpleExecutor {
        pending_threads: Mutex::default(),
    })
}
