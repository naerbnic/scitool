pub(crate) mod cancel;
mod scope;
mod simple;
mod task;
#[cfg(feature = "tokio-exec")]
mod tokio;

use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{event::Event, executor::cancel::IntoCancellableReturn, promise::Promise};

trait DynExecState: Send + Sync {
    fn schedule_sync(&self, parent: Handle, task: Box<dyn FnOnce() + Send>);
    fn schedule_async(&self, parent: Handle, fut: Pin<Box<dyn Future<Output = ()> + Send>>);
    fn start_shut_down(&self) -> &Event;
}

const SHUT_DOWN_MASK: usize = 1 << (usize::BITS - 1);

struct SharedTaskState {
    /// An atomic that tracks both the current active task count, and the
    /// shutting down state in the high bit.
    ///
    /// When shutting down, the count must only decrease.
    active_count: AtomicUsize,
    shutdown_complete: Event,
}

impl SharedTaskState {
    fn new() -> Self {
        Self {
            active_count: AtomicUsize::new(0),
            shutdown_complete: Event::new(),
        }
    }

    /// Starts a new task. Returns `true` if the task was successfully started,
    /// and `false` if the executor is shutting down.
    fn start_new_task(&self) -> bool {
        // Use a fetch loop in order to make an atomic change.
        let mut curr_value = self.active_count.load(Ordering::Relaxed);
        loop {
            if SHUT_DOWN_MASK & curr_value != 0 {
                // We're currently shutting down, so we can't start a new task.
                return false;
            }
            // Increment the task value by one.
            let new_value = curr_value.checked_add(1).unwrap();
            assert_eq!(SHUT_DOWN_MASK & new_value, 0);
            match self.active_count.compare_exchange_weak(
                curr_value,
                new_value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // We successfully incremented the task value.
                    return true;
                }
                Err(e) => {
                    // We failed to increment the task value, so we need to retry.
                    curr_value = e;
                }
            }
        }
    }

    /// Ends a previously started task. This must only be called if `start_new_task()`
    /// returned true previously. Not doing this will likely cause safe but non-local
    /// panics.
    fn end_task(&self) {
        let mut curr_value = self.active_count.load(Ordering::Relaxed);
        // Detect if we're shutting down. Once set, this is never unset, so we don't
        // have to update it after this point.
        let masked_shut_down_value = curr_value & SHUT_DOWN_MASK;
        let is_shutting_down = masked_shut_down_value != 0;
        let was_last = loop {
            let curr_count = curr_value & !SHUT_DOWN_MASK;
            let new_count = curr_count.checked_sub(1).unwrap();
            let new_value = masked_shut_down_value | new_count;
            match self.active_count.compare_exchange_weak(
                curr_value,
                new_value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // We successfully decremented the task value.
                    break new_count == 0;
                }
                Err(e) => {
                    // We failed to decrement the task value, so we need to retry.
                    curr_value = e;
                }
            }
        };

        if was_last && is_shutting_down {
            self.shutdown_complete.signal();
        }
    }

    fn start_shut_down(&self) -> &Event {
        let prev_value = self
            .active_count
            .fetch_or(SHUT_DOWN_MASK, Ordering::Relaxed);
        if prev_value == 0 {
            // The executor wasn't previously shutting down, and it had no
            // active tasks, thus no new tasks can be started on it.
            // Shutdown is already complete.
            self.shutdown_complete.signal();
        }
        &self.shutdown_complete
    }
}

struct TaskGuard<'a>(&'a SharedTaskState);

impl Drop for TaskGuard<'_> {
    fn drop(&mut self) {
        self.0.end_task();
    }
}

struct ExecutorState<Impl>
where
    Impl: ExecutorImpl,
{
    exec_impl: Impl,
    shared_state: Arc<SharedTaskState>,
}

impl<Impl> Drop for ExecutorState<Impl>
where
    Impl: ExecutorImpl,
{
    fn drop(&mut self) {
        let event = self.shared_state.start_shut_down();
        self.exec_impl.block_in_place(|| event.wait_sync());
    }
}

impl<Impl> DynExecState for ExecutorState<Impl>
where
    Impl: ExecutorImpl,
{
    fn schedule_sync(&self, parent: Handle, task: Box<dyn FnOnce() + Send>) {
        self.exec_impl.schedule_sync({
            let shared_state = self.shared_state.clone();
            move || {
                if !shared_state.start_new_task() {
                    // If we failed to start a new task, then the executor
                    // is shutting down. We do not call the function, and leave
                    // it to the caller to detect this by noticing the
                    // task was dropped without running.
                    return;
                }

                let _flavor_guard = scope::WithFlavorGuard::new(scope::ExecutionFlavor::Sync);
                let _handle_guard = scope::WithHandleGuard::new(parent);

                let _task_guard = TaskGuard(&shared_state);

                // We successfully started a new task.
                task();
            }
        });
    }

    fn schedule_async(&self, parent: Handle, task: Pin<Box<dyn Future<Output = ()> + Send>>) {
        self.exec_impl.schedule_async({
            let shared_state = self.shared_state.clone();
            async move {
                if !shared_state.start_new_task() {
                    // If we failed to start a new task, then the executor
                    // is shutting down. We do not call the function, and leave
                    // it to the caller to detect this by noticing the
                    // task was dropped without running.
                    return;
                }
                let _task_guard = TaskGuard(&shared_state);
                scope::WithHandleFuture::new(parent, scope::WithAsyncFlavorFuture::new(task)).await;
            }
        });
    }

    fn start_shut_down(&self) -> &Event {
        self.shared_state.start_shut_down()
    }
}

#[derive(Clone)]
pub(crate) struct Handle {
    state: Arc<dyn DynExecState>,
}

impl Handle {
    #[expect(dead_code)]
    pub(crate) fn schedule<F, R>(&self, func: F) -> Promise<R::Output>
    where
        F: FnOnce() -> R + Send + 'static,
        R: IntoCancellableReturn,
        R::Output: Send + 'static,
    {
        let (promise, fulfiller) = Promise::new();
        self.state.schedule_sync(
            self.clone(),
            Box::new(move || match func().into_cancellable_return() {
                Ok(value) => {
                    // We don't care if the sender is cancelled here.
                    let _ = fulfiller.fulfill(value);
                }

                // Dropping the fulfiller signals cancellation.
                Err(_) => drop(fulfiller),
            }),
        );
        promise
    }

    pub(crate) fn schedule_async<F, R>(&self, fut: F) -> Promise<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (promise, fulfiller) = Promise::new();
        self.state.schedule_async(
            self.clone(),
            Box::pin(async move {
                fulfiller.fulfill(fut.await);
            }),
        );
        promise
    }

    pub(crate) fn try_current() -> Option<Self> {
        scope::try_current()
    }

    pub(crate) fn current() -> Self {
        Self::try_current().expect("requires running in an executor scope")
    }
}

#[repr(transparent)]
pub(crate) struct Executor {
    state: Arc<dyn DynExecState>,
}

impl Executor {
    pub(crate) fn new<I>(exec_impl: I) -> Self
    where
        I: ExecutorImpl,
    {
        Self {
            state: Arc::new(ExecutorState {
                exec_impl,
                shared_state: Arc::new(SharedTaskState::new()),
            }),
        }
    }

    pub(crate) fn handle(&self) -> Handle {
        Handle {
            state: self.state.clone(),
        }
    }

    #[expect(dead_code)]
    pub(crate) fn schedule<F, R>(&self, func: F) -> Promise<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (promise, fulfiller) = Promise::new();
        self.state.schedule_sync(
            self.handle(),
            Box::new(move || {
                fulfiller
                    .fulfill(func())
                    .expect("FIXME: Handle Cancellation")
            }),
        );
        promise
    }

    pub(crate) fn schedule_async<F, R>(&self, fut: F) -> Promise<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (promise, fulfiller) = Promise::new();
        self.state.schedule_async(
            self.handle(),
            Box::pin(async move {
                fulfiller.fulfill(fut.await);
            }),
        );
        promise
    }

    #[expect(dead_code)]
    pub(crate) fn shutdown(&self) -> &Event {
        self.state.start_shut_down()
    }
}

impl Executor {
    pub(crate) fn new_simple() -> Self {
        simple::make_simple()
    }

    #[cfg(feature = "tokio-exec")]
    pub(crate) fn new_tokio(handle: &asynx::deps::tokio::runtime::Handle) -> Self {
        Self::new(tokio::TokioExecutor::new(handle))
    }
}

pub(crate) trait ExecutorImpl: Send + Sync + 'static {
    fn schedule_sync<F>(&self, func: F)
    where
        F: FnOnce() + Send + 'static;

    fn schedule_async<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static;

    /// Run the given synchronous function, and do not return until
    /// the function returns.
    ///
    /// This may or may not invoke the executor itself, but if this is running
    /// in the executor, calling this should generally prevent deadlock if the
    /// current task is waiting for some other task to complete.
    fn block_in_place<F>(&self, func: F)
    where
        F: FnOnce();
}
