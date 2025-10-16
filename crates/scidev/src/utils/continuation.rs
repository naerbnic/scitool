use std::{
    cell::RefCell,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake},
    thread::Thread,
};

struct WaitInner {
    curr_thread: Option<Thread>,
    is_ready: bool,
    waiter_dropped: bool,
    num_notifiers: usize,
}

struct WaitToken {
    inner: Arc<Mutex<WaitInner>>,
}

impl WaitToken {
    pub(crate) fn new() -> (Self, NotifyToken) {
        let inner = Arc::new(Mutex::new(WaitInner {
            curr_thread: None,
            is_ready: false,
            waiter_dropped: false,
            num_notifiers: 1,
        }));
        (
            Self {
                inner: inner.clone(),
            },
            NotifyToken { inner },
        )
    }

    pub(crate) fn wait(&self) {
        let mut guard = self.inner.lock().unwrap();
        if guard.is_ready {
            return;
        }
        guard.curr_thread = Some(std::thread::current());
        drop(guard);
        loop {
            std::thread::park();
            let guard = self.inner.lock().unwrap();
            if guard.is_ready {
                break;
            }
            // Now we know that all notifiers have been dropped and we are not ready, so if
            // we park again, we will deadlock.
            //
            // It's unclear if we should panic or just return. For sanity, we
            // will panic.
            assert!(
                guard.num_notifiers != 0,
                "Notifiers dropped without notifying."
            );
        }
    }
}

impl Drop for WaitToken {
    fn drop(&mut self) {
        let mut guard = self.inner.lock().unwrap();
        guard.num_notifiers -= 1;
    }
}

struct NotifyToken {
    inner: Arc<Mutex<WaitInner>>,
}

impl NotifyToken {
    pub(crate) fn notify(&self) {
        let mut guard = self.inner.lock().unwrap();
        if guard.is_ready {
            return;
        }
        guard.is_ready = true;
        if let Some(thread) = guard.curr_thread.take() {
            thread.unpark();
        }
    }
}

impl Wake for NotifyToken {
    fn wake(self: Arc<Self>) {
        self.notify();
    }
}

impl Clone for NotifyToken {
    fn clone(&self) -> Self {
        let mut guard = self.inner.lock().unwrap();
        guard.num_notifiers = guard
            .num_notifiers
            .checked_add(1)
            .expect("Overflow of number of notifiers");
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Drop for NotifyToken {
    fn drop(&mut self) {
        let mut guard = self.inner.lock().unwrap();
        guard.waiter_dropped = true;
        if let Some(thread) = guard.curr_thread.take() {
            thread.unpark();
        }
    }
}

#[derive(Debug)]
enum FlowState<In, Out> {
    BeforeStarted,
    // The computation is ready with an output value
    Ready(Out),
    // The computation is currently running, and has not yet produced an output
    Running,
    // The computation has been provided with input and should continue
    Continue(In),
    // The computation is currently paused, waiting for more input
    Paused,
    // The computation has finished and will produce no more output
    Finished,
}

#[derive(Debug)]
struct Inner<In, Out> {
    flow_state: FlowState<In, Out>,
}

/// A channel that sends results and receives the next input for a continuation.
#[derive(Debug, Clone)]
pub struct Channel<In, Out> {
    /// Note that we explicitly reverse the direction of In and Out here, since
    /// from the perspective of the continuation, it is receiving Out and sending In.
    inner: Rc<RefCell<Inner<Out, In>>>,
}

impl<In, Out> Channel<In, Out> {
    /// Yields a value to the caller, and waits for the next input.
    ///
    /// This returns control to the caller, and the future will not
    /// make progress until the caller calls `next` with a new input value.
    ///
    /// You must await the returned future for correctness. Dropping it
    /// without awaiting will cause a panic within the async context.
    pub fn yield_value(&mut self, value: In) -> ChannelYield<In, Out> {
        ChannelYield {
            inner: self.inner.clone(),
            input: Some(value),
            is_handled: false,
        }
    }
}

#[derive(Debug)]
/// A future that waits for a response from the channel.
pub struct ChannelYield<In, Out> {
    /// The channel context we are waiting on.
    inner: Rc<RefCell<Inner<Out, In>>>,
    input: Option<In>,
    is_handled: bool,
}

impl<In, Out> Future for ChannelYield<In, Out>
where
    In: Unpin,
    Out: Unpin,
{
    type Output = Out;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Out> {
        let this = &mut *self;
        let mut guard = this.inner.borrow_mut();
        let flow_state = &mut guard.flow_state;

        if let Some(value) = this.input.take() {
            assert!(
                matches!(flow_state, FlowState::Running),
                "Yield called when not running"
            );
            *flow_state = FlowState::Ready(value);
            return Poll::Pending;
        }

        match std::mem::replace(flow_state, FlowState::Running) {
            FlowState::Continue(input) => {
                *flow_state = FlowState::Running;
                drop(guard);
                self.is_handled = true;
                Poll::Ready(input)
            }
            FlowState::Finished => panic!("Cannot continue a finished continuation"),
            FlowState::Running => panic!("Cannot yield when already running"),
            state => {
                // This is a sound state to be in, but no progress has been
                // made. Restore state and yield again.
                *flow_state = state;
                Poll::Pending
            }
        }
    }
}

impl<In, Out> Drop for ChannelYield<In, Out> {
    fn drop(&mut self) {
        if self.is_handled {
            return;
        }
        let guard = self.inner.borrow();
        let flow_state = &guard.flow_state;

        // If this is dropped while paused, then this is due to future
        // cancellation. If not, then this is a logic error, as we have sent
        // a value but not received a response.
        //
        // This should never be triggered if the ChannelYield future is
        // .await-ed properly.
        assert!(
            matches!(flow_state, FlowState::Paused),
            "Dropping ChannelResult without consuming the yielded value"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuationResult<Out, Result> {
    Yield(Out),
    Complete(Result),
}

impl<Out, Result> ContinuationResult<Out, Result> {
    pub fn as_complete(&self) -> Option<&Result> {
        match self {
            ContinuationResult::Yield(_) => None,
            ContinuationResult::Complete(result) => Some(result),
        }
    }

    pub fn into_complete(self) -> Option<Result> {
        match self {
            ContinuationResult::Yield(_) => None,
            ContinuationResult::Complete(result) => Some(result),
        }
    }

    pub fn as_yield(&self) -> Option<&Out> {
        match self {
            ContinuationResult::Yield(value) => Some(value),
            ContinuationResult::Complete(_) => None,
        }
    }

    pub fn into_yield(self) -> Option<Out> {
        match self {
            ContinuationResult::Yield(value) => Some(value),
            ContinuationResult::Complete(_) => None,
        }
    }
}

enum ContinuationPoll<Out, Result> {
    Ready(ContinuationResult<Out, Result>),
    Pending(WaitToken),
}

pub struct Continuation<'a, In, Out, Result> {
    state: Rc<RefCell<Inner<In, Out>>>,
    curr_future: Option<Pin<Box<dyn Future<Output = Result> + 'a>>>,
}

impl<'a, In, Out, Result> Continuation<'a, In, Out, Result> {
    /// Potentially run a single poll of the continuation
    fn poll_continuation(&mut self) -> Option<ContinuationPoll<Out, Result>> {
        let curr_future = self.curr_future.as_mut()?;
        {
            let mut guard = self.state.borrow_mut();
            let flow_state = &mut guard.flow_state;

            match std::mem::replace(flow_state, FlowState::Running) {
                FlowState::Ready(value) => {
                    *flow_state = FlowState::Paused;
                    return Some(ContinuationPoll::Ready(ContinuationResult::Yield(value)));
                }
                FlowState::Finished => {
                    unreachable!("If this is finished, curr_future should be None");
                }
                FlowState::Paused => unreachable!("Cannot poll a paused continuation"),
                FlowState::BeforeStarted => {
                    unreachable!("Continuation polled before starting");
                }
                state => {
                    // We are still waiting on the future to provide the next value,
                    // or to finish. Restore state and continue.
                    *flow_state = state;
                }
            }
        }
        let (wait_token, notify_token) = WaitToken::new();
        let waker = Arc::new(notify_token).into();
        let mut cx = Context::from_waker(&waker);
        match curr_future.as_mut().poll(&mut cx) {
            Poll::Ready(result) => {
                let mut guard = self.state.borrow_mut();
                guard.flow_state = FlowState::Finished;
                self.curr_future = None;
                Some(ContinuationPoll::Ready(ContinuationResult::Complete(
                    result,
                )))
            }
            Poll::Pending => {
                // We have to check our state to see if this is intended to
                // be a yield, or if some other future registered the waker
                // elsewhere.
                let mut guard = self.state.borrow_mut();
                let flow_state = &mut guard.flow_state;
                match std::mem::replace(flow_state, FlowState::Paused) {
                    FlowState::Running => {
                        // We didn't change states, so another future should have
                        // installed the waker somewhere. Signal to caller.
                        *flow_state = FlowState::Running;
                        Some(ContinuationPoll::Pending(wait_token))
                    }
                    FlowState::Ready(value) => {
                        // We have a value ready, so return it.
                        Some(ContinuationPoll::Ready(ContinuationResult::Yield(value)))
                    }
                    FlowState::Finished | FlowState::Paused => {
                        unreachable!("Unexpected state after polling continuation future");
                    }
                    FlowState::Continue(_) => {
                        unreachable!("Value was not consumed as expected.");
                    }
                    FlowState::BeforeStarted => {
                        unreachable!("Continuation polled before starting");
                    }
                }
            }
        }
    }

    pub fn pump_continuation(&mut self) -> Option<ContinuationResult<Out, Result>> {
        loop {
            match self.poll_continuation()? {
                ContinuationPoll::Ready(poll) => return Some(poll),
                ContinuationPoll::Pending(waiter) => {
                    // Wait for the waker to be notified
                    waiter.wait();
                }
            }
        }
    }

    pub fn new<F, Fut>(future_fn: F) -> Self
    where
        F: FnOnce(Channel<Out, In>) -> Fut,
        Fut: Future<Output = Result> + 'a,
    {
        let state = Rc::new(RefCell::new(Inner {
            flow_state: FlowState::BeforeStarted,
        }));

        let channel = Channel {
            inner: state.clone(),
        };

        let curr_future = Box::pin(future_fn(channel));

        Self {
            state,
            curr_future: Some(curr_future),
        }
    }

    #[must_use]
    pub fn has_started(&self) -> bool {
        let guard = self.state.borrow();
        !matches!(guard.flow_state, FlowState::BeforeStarted)
    }

    pub fn start(&mut self) -> ContinuationResult<Out, Result> {
        {
            let mut guard = self.state.borrow_mut();
            assert!(
                matches!(guard.flow_state, FlowState::BeforeStarted),
                "Continuation already started"
            );
            guard.flow_state = FlowState::Running;
        }
        self.pump_continuation()
            .expect("Continuation marked as finished immediately")
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        let guard = self.state.borrow();
        matches!(guard.flow_state, FlowState::Finished)
    }

    pub fn next(&mut self, input: In) -> ContinuationResult<Out, Result> {
        {
            let mut guard = self.state.borrow_mut();
            let flow_state = &mut guard.flow_state;

            assert!(
                matches!(flow_state, FlowState::Paused),
                "In unexpected state when continuing"
            );

            *flow_state = FlowState::Continue(input);
        }

        self.pump_continuation()
            .expect("Next called on finished continuation")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PendingOnce {
        has_pending: bool,
    }

    impl Future for PendingOnce {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.has_pending {
                self.has_pending = false;
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        }
    }

    #[test]
    fn test_no_pause() {
        let mut cont = Continuation::new(|_channel: Channel<u32, u32>| async move { 42 });
        assert_eq!(cont.start().as_complete(), Some(&42));
    }

    #[test]
    fn test_unrelated_future() {
        let mut cont = Continuation::new(|_channel: Channel<u32, u32>| async move {
            let pending_once = PendingOnce { has_pending: true };
            pending_once.await;
            3
        });
        assert_eq!(cont.start().as_complete(), Some(&3));
    }

    #[test]
    fn test_simple_yield() {
        let mut cont = Continuation::new(|mut channel: Channel<u32, u32>| async move {
            let val = channel.yield_value(1).await;
            assert_eq!(val, 2);
            let val = channel.yield_value(3).await;
            assert_eq!(val, 4);
            42
        });
        assert_eq!(cont.start().into_yield(), Some(1));
        assert_eq!(cont.next(2).into_yield(), Some(3));
        assert_eq!(cont.next(4).into_complete(), Some(42));
    }
}
