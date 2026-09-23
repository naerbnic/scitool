mod node;
mod task_waker;

use std::{
    num::NonZero,
    task::{Context, Poll, ready},
};

use crate::event::wait_node::{
    node::{InputPolicy, NodeRef, NodeState, OutputPolicy, RefPolicy, WaitSetRef, create_node},
    task_waker::TaskWaker,
};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("The handle backing has been dropped.")]
pub(crate) struct DroppedError;

struct WakerState<P>
where
    P: RefPolicy,
{
    /// Handle to a node that contains the current state.
    token: TaskWaker,
    /// Node this waker is registered with.
    handle: NodeRef<P>,
    registered: bool,
}

impl<P> WakerState<P>
where
    P: RefPolicy,
{
    pub(crate) fn poll_resolved(&mut self, cx: &mut Context<'_>) -> Poll<NodeState> {
        if let Some(state) = self.handle.state() {
            return Poll::Ready(state);
        }

        self.register();

        if let Some(state) = self.handle.state() {
            Poll::Ready(state)
        } else {
            self.token.set_waker(cx);
            Poll::Pending
        }
    }

    fn register(&mut self) {
        if !self.registered {
            self.handle.add_token(self.token.clone());
            self.registered = true;
        }
    }

    fn unregister(&mut self) {
        if self.registered {
            self.handle.remove_token(self.token.id());
            self.registered = false;
        }
    }

    fn new(handle: NodeRef<P>) -> Self {
        Self {
            token: TaskWaker::new(),
            handle,
            registered: false,
        }
    }

    fn id(&self) -> NonZero<usize> {
        self.token.id()
    }
}

impl<P> Clone for WakerState<P>
where
    P: RefPolicy,
{
    fn clone(&self) -> Self {
        Self::new(self.handle.clone())
    }
}

impl<P> Drop for WakerState<P>
where
    P: RefPolicy,
{
    fn drop(&mut self) {
        self.unregister();
    }
}

#[derive(Clone)]
pub(crate) struct EventToken {
    state: WakerState<OutputPolicy>,
}

impl EventToken {
    fn new_impl(handle: NodeRef<OutputPolicy>) -> Self {
        Self {
            state: WakerState::new(handle),
        }
    }

    pub(crate) fn new() -> (Self, EventTrigger) {
        let (input, output) = create_node();
        (Self::new_impl(output), EventTrigger::new_impl(input))
    }

    pub(crate) fn new_triggered() -> Self {
        Self::new_impl(NodeRef::new_resolved(NodeState::Triggered))
    }

    pub(crate) fn new_dropped() -> Self {
        Self::new_impl(NodeRef::new_resolved(NodeState::Dropped))
    }

    pub(crate) fn poll_triggered(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), DroppedError>> {
        let node_state = ready!(self.state.poll_resolved(cx));
        Poll::Ready(match node_state {
            NodeState::Dropped => Err(DroppedError),
            NodeState::Triggered => Ok(()),
        })
    }

    pub(crate) fn connect(self, other: EventTrigger) {
        self.state
            .handle
            .clone()
            .connect(other.state.handle.clone());
    }

    pub(crate) fn wait_sync(&mut self) -> Result<(), DroppedError> {
        asynx::executor::block_on_poller(|cx| self.poll_triggered(cx))
    }

    pub(crate) fn try_wait(&mut self) -> Option<Result<(), DroppedError>> {
        Some(match self.state.handle.state()? {
            NodeState::Dropped => Err(DroppedError),
            NodeState::Triggered => Ok(()),
        })
    }
}

impl Future for EventToken {
    type Output = Result<(), DroppedError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.as_mut().poll_triggered(cx)
    }
}

#[derive(Clone)]
pub(crate) struct EventTrigger {
    state: WakerState<InputPolicy>,
}

impl EventTrigger {
    fn new_impl(handle: NodeRef<InputPolicy>) -> Self {
        Self {
            state: WakerState::new(handle),
        }
    }

    pub(crate) fn trigger(&self) -> Result<(), DroppedError> {
        if self.state.handle.trigger() {
            Ok(())
        } else {
            Err(DroppedError)
        }
    }

    pub(crate) fn poll_resolved(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        ready!(self.state.poll_resolved(cx));
        Poll::Ready(())
    }

    pub(crate) fn connect(self, other: EventToken) {
        other
            .state
            .handle
            .clone()
            .connect(self.state.handle.clone());
    }

    pub(crate) fn token_id(&self) -> NonZero<usize> {
        self.state.id()
    }
}

#[derive(Clone)]
pub(crate) struct WaitSet(WaitSetRef<InputPolicy>);

impl WaitSet {
    pub(crate) fn new() -> Self {
        Self(WaitSetRef::new())
    }

    pub(crate) fn add_trigger(&self, token: &EventTrigger) {
        self.0.add_node(&token.state.handle);
    }

    pub(crate) fn trigger_next(&self) -> bool {
        self.0.trigger_next()
    }

    pub(crate) fn trigger_all(&self) -> usize {
        self.0.trigger_all()
    }
}
#[cfg(test)]
mod tests {
    use std::{future::poll_fn, task::Waker, thread, time::Duration};

    use super::*;

    #[test]
    fn test_basic_trigger() {
        let (mut token, trigger) = EventToken::new();
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert_eq!(token.poll_triggered(&mut cx), Poll::Pending);
        assert!(trigger.trigger().is_ok());
        assert_eq!(token.poll_triggered(&mut cx), Poll::Ready(Ok(())));
    }

    #[test]
    fn test_trigger_dropped() {
        let (mut token, trigger) = EventToken::new();
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert_eq!(token.poll_triggered(&mut cx), Poll::Pending);
        drop(trigger);
        assert!(matches!(
            token.poll_triggered(&mut cx),
            Poll::Ready(Err(DroppedError))
        ));
    }

    #[test]
    fn test_token_dropped() {
        let (token, trigger) = EventToken::new();
        drop(token);
        assert!(trigger.trigger().is_err());
    }

    #[test]
    fn test_multiple_triggers_drop_one() {
        let (mut token, trigger1) = EventToken::new();
        let trigger2 = trigger1.clone();
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        drop(trigger1);
        assert_eq!(token.poll_triggered(&mut cx), Poll::Pending);

        assert!(trigger2.trigger().is_ok());
        assert_eq!(token.poll_triggered(&mut cx), Poll::Ready(Ok(())));
    }

    #[test]
    fn test_multiple_tokens_all_receive_trigger() {
        let (mut token1, trigger) = EventToken::new();
        let mut token2 = token1.clone();
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert_eq!(token1.poll_triggered(&mut cx), Poll::Pending);
        assert_eq!(token2.poll_triggered(&mut cx), Poll::Pending);

        assert!(trigger.trigger().is_ok());

        assert_eq!(token1.poll_triggered(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(token2.poll_triggered(&mut cx), Poll::Ready(Ok(())));
    }

    #[test]
    fn test_forward_connect_trigger_propagation() {
        let (token_a, trigger_a) = EventToken::new();
        let (mut token_b, trigger_b) = EventToken::new();
        token_a.connect(trigger_b);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert_eq!(token_b.poll_triggered(&mut cx), Poll::Pending);
        assert!(trigger_a.trigger().is_ok());
        assert_eq!(token_b.poll_triggered(&mut cx), Poll::Ready(Ok(())));
    }

    #[test]
    fn test_forward_connect_drop_propagation() {
        let (token_a, trigger_a) = EventToken::new();
        let (mut token_b, trigger_b) = EventToken::new();
        token_a.connect(trigger_b);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert_eq!(token_b.poll_triggered(&mut cx), Poll::Pending);
        drop(trigger_a);
        assert!(matches!(
            token_b.poll_triggered(&mut cx),
            Poll::Ready(Err(DroppedError))
        ));
    }

    #[test]
    fn test_backward_connect_cancellation() {
        let (token_a, trigger_a) = EventToken::new();
        let (token_b, trigger_b) = EventToken::new();
        token_a.connect(trigger_b);

        drop(token_b);
        assert!(trigger_a.trigger().is_err());
    }

    #[test]
    fn test_async_thread_wakeup() {
        let (mut token, trigger) = EventToken::new();

        let handle = thread::spawn(move || {
            futures_executor::block_on(async move {
                poll_fn(|cx| token.poll_triggered(cx)).await.unwrap();
            });
        });

        thread::sleep(Duration::from_millis(20));
        assert!(trigger.trigger().is_ok());
        handle.join().unwrap();
    }

    #[test]
    fn test_wait_set_basic_fifo() {
        let (mut token1, trigger1) = EventToken::new();
        let (mut token2, trigger2) = EventToken::new();
        let (mut token3, trigger3) = EventToken::new();
        let wait_set = WaitSet::new();

        wait_set.add_trigger(&trigger1);
        wait_set.add_trigger(&trigger2);
        wait_set.add_trigger(&trigger3);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert_eq!(token1.poll_triggered(&mut cx), Poll::Pending);
        assert_eq!(token2.poll_triggered(&mut cx), Poll::Pending);
        assert_eq!(token3.poll_triggered(&mut cx), Poll::Pending);

        assert!(wait_set.trigger_next());
        assert_eq!(token1.poll_triggered(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(token2.poll_triggered(&mut cx), Poll::Pending);
        assert_eq!(token3.poll_triggered(&mut cx), Poll::Pending);

        assert!(wait_set.trigger_next());
        assert_eq!(token2.poll_triggered(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(token3.poll_triggered(&mut cx), Poll::Pending);

        assert!(wait_set.trigger_next());
        assert_eq!(token3.poll_triggered(&mut cx), Poll::Ready(Ok(())));

        assert!(!wait_set.trigger_next());
    }

    #[test]
    fn test_wait_set_trigger_next_skips_dropped_waiter() {
        let (token1, trigger1) = EventToken::new();
        let (mut token2, trigger2) = EventToken::new();
        let wait_set = WaitSet::new();

        wait_set.add_trigger(&trigger1);
        wait_set.add_trigger(&trigger2);

        // Drop the first waiter's token so its node resolves to Dropped.
        drop(token1);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        // trigger_next should loop past the dropped waiter and trigger the live one.
        assert!(wait_set.trigger_next());
        assert_eq!(token2.poll_triggered(&mut cx), Poll::Ready(Ok(())));
        assert!(!wait_set.trigger_next());
    }

    #[test]
    fn test_wait_set_automatic_eviction_on_token_drop() {
        let (token1, trigger1) = EventToken::new();
        let wait_set = WaitSet::new();

        wait_set.add_trigger(&trigger1);
        drop(token1);

        // After dropping the token, the trigger should be evicted, leaving wait_set empty.
        assert!(!wait_set.trigger_next());
    }

    #[test]
    fn test_wait_set_add_already_dropped_trigger() {
        let (token1, trigger1) = EventToken::new();
        drop(token1);

        let (mut token2, trigger2) = EventToken::new();
        let wait_set = WaitSet::new();

        // Adding an already-dropped trigger should fail add_edge and roll back.
        wait_set.add_trigger(&trigger1);
        wait_set.add_trigger(&trigger2);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert!(wait_set.trigger_next());
        assert_eq!(token2.poll_triggered(&mut cx), Poll::Ready(Ok(())));
        assert!(!wait_set.trigger_next());
    }

    #[test]
    fn test_wait_set_duplicate_add() {
        let (mut token, trigger) = EventToken::new();
        let wait_set = WaitSet::new();

        // Adding the same trigger twice should deduplicate and not panic.
        wait_set.add_trigger(&trigger);
        wait_set.add_trigger(&trigger);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert!(wait_set.trigger_next());
        assert_eq!(token.poll_triggered(&mut cx), Poll::Ready(Ok(())));
        assert!(!wait_set.trigger_next());
    }

    #[test]
    fn test_wait_set_trigger_all_accounting() {
        let (mut token1, trigger1) = EventToken::new();
        let (token2, trigger2) = EventToken::new();
        let (mut token3, trigger3) = EventToken::new();
        let wait_set = WaitSet::new();

        wait_set.add_trigger(&trigger1);
        wait_set.add_trigger(&trigger2);
        wait_set.add_trigger(&trigger3);

        // Drop token2 so that trigger2 becomes dead.
        drop(token2);

        // trigger_all should trigger the remaining 2 live waiters and return 2.
        assert_eq!(wait_set.trigger_all(), 2);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert_eq!(token1.poll_triggered(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(token3.poll_triggered(&mut cx), Poll::Ready(Ok(())));
        assert!(!wait_set.trigger_next());
    }

    #[test]
    fn test_wait_set_clone_shares_queue() {
        let (mut token1, trigger1) = EventToken::new();
        let (mut token2, trigger2) = EventToken::new();
        let wait_set1 = WaitSet::new();
        let wait_set2 = wait_set1.clone();

        wait_set1.add_trigger(&trigger1);
        wait_set2.add_trigger(&trigger2);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        assert!(wait_set2.trigger_next());
        assert_eq!(token1.poll_triggered(&mut cx), Poll::Ready(Ok(())));

        assert!(wait_set1.trigger_next());
        assert_eq!(token2.poll_triggered(&mut cx), Poll::Ready(Ok(())));

        assert!(!wait_set1.trigger_next());
        assert!(!wait_set2.trigger_next());
    }
}

