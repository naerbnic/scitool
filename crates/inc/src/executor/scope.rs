use std::cell::{Cell, RefCell};

use crate::executor::Handle;

thread_local! {
    static EXEC_SCOPE: Scope = const { Scope { task_flavor: Cell::new(None), curr_handle: RefCell::new(None)} };
}

/// The mode that some code is executing or will be executed in.
#[derive(Clone, Copy, Debug)]
pub(super) enum ExecutionFlavor {
    Sync,
    Async,
}

struct Scope {
    /// If the current thread is running within a given executor, the type of
    /// executor mode it is running in. Note that this does not always match
    /// with the current function's sync/asyncness: A future can be polled on
    /// a sychronous thread using `block_on`, while async code often calls
    /// non-async code.
    ///
    /// This can be used to ensure that we don't "leak" execution styles on
    /// different threads within the executor.
    ///
    /// If this is `None`, then our current execution is not dependent on the
    /// current executor, but is still made available through the current
    /// thread scope.
    task_flavor: Cell<Option<ExecutionFlavor>>,

    /// A pointer to the current executor. This must be owned by some layer of
    /// the current execution scope, or must be static.
    curr_handle: RefCell<Option<Handle>>,
}

pub(super) struct WithFlavorGuard {
    old_flavor: Option<ExecutionFlavor>,
}

impl WithFlavorGuard {
    pub(super) fn new(flavor: ExecutionFlavor) -> Self {
        let old_flavor = EXEC_SCOPE.with(|scope| scope.task_flavor.replace(Some(flavor)));
        Self { old_flavor }
    }
}

impl Drop for WithFlavorGuard {
    fn drop(&mut self) {
        EXEC_SCOPE.with(|scope| scope.task_flavor.replace(self.old_flavor));
    }
}

#[pin_project::pin_project]
pub(super) struct WithAsyncFlavorFuture<F> {
    #[pin]
    child: F,
}

impl<F> WithAsyncFlavorFuture<F> {
    pub(super) fn new(child: F) -> Self {
        Self { child }
    }
}

impl<F> Future for WithAsyncFlavorFuture<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let _guard = WithFlavorGuard::new(ExecutionFlavor::Async);
        self.project().child.poll(cx)
    }
}

pub(super) struct WithHandleGuard {
    old_handle: Option<Handle>,
}

impl WithHandleGuard {
    pub(super) fn new(handle: Handle) -> Self {
        let old_handle = EXEC_SCOPE.with(|scope| scope.curr_handle.replace(Some(handle)));
        Self { old_handle }
    }
}

impl Drop for WithHandleGuard {
    fn drop(&mut self) {
        EXEC_SCOPE.with(|scope| {
            let mut guard = scope.curr_handle.borrow_mut();
            *guard = self.old_handle.take();
        });
    }
}

#[pin_project::pin_project]
pub(super) struct WithHandleFuture<F> {
    handle: Handle,
    #[pin]
    child: F,
}

impl<F> WithHandleFuture<F> {
    pub(super) fn new(handle: Handle, child: F) -> Self {
        Self { handle, child }
    }
}

impl<F> Future for WithHandleFuture<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let _guard = WithHandleGuard::new(self.handle.clone());
        self.project().child.poll(cx)
    }
}

pub(super) fn try_current() -> Option<Handle> {
    EXEC_SCOPE.with(|scope| scope.curr_handle.borrow().clone())
}

fn is_in_async() -> bool {
    // We assume if we are not in an executor, or in a block_on call, we're in a
    // sync context.
    EXEC_SCOPE.with(|scope| matches!(scope.task_flavor.get(), Some(ExecutionFlavor::Async)))
}

pub(super) fn assert_in_sync() {
    assert!(
        !is_in_async(),
        "Operation expects to be executed in a synchronous scope."
    );
}

pub(super) fn assert_in_async() {
    assert!(
        is_in_async(),
        "Operation expects to be executed in an asynchronous scope."
    );
}
