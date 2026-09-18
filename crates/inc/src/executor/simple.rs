use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::{JoinHandle, ThreadId},
};

use crate::executor::{Executor, ExecutorImpl};

pub(crate) struct SimpleExecutor {
    pending_threads: Arc<Mutex<HashMap<ThreadId, JoinHandle<()>>>>,
}

impl ExecutorImpl for SimpleExecutor {
    fn schedule_impl(&self, func: Box<dyn FnOnce() + Send + 'static>) {
        let mut pending = self.pending_threads.lock().unwrap();
        let handle = std::thread::spawn({
            let pending_threads = self.pending_threads.clone();
            move || {
                func();
                // We can try to remove our own ThreadID from the map. If it
                // succeeds, no one is trying to join us. Since we are about
                // to exit, there is no need to join further.
                pending_threads
                    .lock()
                    .unwrap()
                    .remove(&std::thread::current().id());
            }
        });
        pending.insert(handle.thread().id(), handle);
    }

    fn schedule_async_impl(
        &self,
        fut: std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) {
        self.schedule_impl(Box::new(move || futures_executor::block_on(fut)));
    }

    fn shutdown(self: Box<Self>) {
        let SimpleExecutor { pending_threads } = *self;
        let pending = std::mem::take(&mut *pending_threads.lock().unwrap());
        #[allow(clippy::iter_over_hash_type)]
        for (_, handle) in pending {
            handle.join().expect("Failed to join thread");
        }
    }
}

pub(crate) fn make_simple() -> Executor {
    Executor::new(SimpleExecutor {
        pending_threads: Arc::new(Mutex::default()),
    })
}
