//! Utilities allowing for a thread/task to report if it is in an async scope.

use std::cell::Cell;

/// The mode that some code is executing or will be executed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExecutionFlavor {
    Sync,
    Async,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FlavorState {
    curr_flavor: ExecutionFlavor,
    stack_depth: usize,
}

thread_local! {
    static CURR_FLAVOR: Cell<FlavorState> = const {
        Cell::new(FlavorState {
            curr_flavor: ExecutionFlavor::Sync,
            stack_depth: 0,
        })
    };
}

pub(super) struct WithFlavorGuard {
    old_state: FlavorState,
    expected_state: FlavorState,
}

impl WithFlavorGuard {
    fn new(flavor: ExecutionFlavor) -> Self {
        let (old_state, expected_state) = CURR_FLAVOR.with(|curr_flavor| {
            let old = curr_flavor.get();
            let new = FlavorState {
                curr_flavor: flavor,
                stack_depth: old.stack_depth + 1,
            };
            curr_flavor.set(new);
            (old, new)
        });
        Self {
            old_state,
            expected_state,
        }
    }
}

impl Drop for WithFlavorGuard {
    fn drop(&mut self) {
        CURR_FLAVOR.with(|curr_flavor| {
            let prev_state = curr_flavor.replace(self.old_state);
            assert_eq!(prev_state, self.expected_state);
        });
    }
}

pub fn in_sync_scope<F, Output>(f: F) -> Output
where
    F: FnOnce() -> Output,
{
    let _guard = WithFlavorGuard::new(ExecutionFlavor::Sync);
    f()
}

/// A future that enables the async flavor while in the scope of the future.
///
/// This can be used in executors to easily add the async flavor
/// while executing the task.
#[pin_project::pin_project]
pub struct WithAsyncFlavorFuture<F> {
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

pub fn run_wth_async_scope<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = WithFlavorGuard::new(ExecutionFlavor::Async);
    f()
}

pub fn with_async_scope<F>(f: F) -> WithAsyncFlavorFuture<F>
where
    F: Future,
{
    WithAsyncFlavorFuture::new(f)
}

fn is_in_scope(scope: ExecutionFlavor) -> bool {
    CURR_FLAVOR.with(|curr_flavor| curr_flavor.get().curr_flavor == scope)
}

pub fn assert_in_sync() {
    assert!(
        is_in_scope(ExecutionFlavor::Sync),
        "Operation expects to be executed in a synchronous scope."
    );
}

pub fn assert_in_async() {
    assert!(
        is_in_scope(ExecutionFlavor::Async),
        "Operation expects to be executed in an asynchronous scope."
    );
}
