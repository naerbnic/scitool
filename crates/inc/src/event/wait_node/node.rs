use std::{
    collections::{BTreeMap, HashMap, btree_map, hash_map},
    num::NonZero,
    sync::{Arc, Mutex},
};

use crate::event::wait_node::TaskWaker;

#[derive(Clone, Copy)]
pub(super) enum NodeState {
    Dropped,
    Triggered,
}

enum Edge {
    Token(TaskWaker),
    IncomingNode(NodeRef<OutputPolicy>),
    OutgoingNode(NodeRef<InputPolicy>),
    WaitSet(WaitSetRef<OutputPolicy>),
}

impl Edge {
    fn trigger(&self) {
        if let Self::OutgoingNode(handle) = &self {
            handle.trigger();
        }
    }

    /// Drop the given node from the link target, if suitable.
    fn drop_node(&self, node: NonZero<usize>) {
        match self {
            Self::IncomingNode(out_handle) => out_handle.remove_edge(node),
            Self::OutgoingNode(in_handle) => in_handle.remove_edge(node),
            Self::Token(token) => {
                // If this is called, the node must be resolved, so we are
                // ready to be read.
                token.wake();
            }
            Self::WaitSet(wait_set) => {
                wait_set.remove_edge(node);
            }
        }
    }

    fn id(&self) -> NonZero<usize> {
        match self {
            Self::Token(token) => token.id(),
            Self::IncomingNode(handle) => handle.id(),
            Self::OutgoingNode(handle) => handle.id(),
            Self::WaitSet(wait_set) => wait_set.id(),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey(NonZero<usize>);

#[derive(Default)]
struct EdgeSet(BTreeMap<EdgeKey, Edge>);

impl EdgeSet {
    fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Trigger all of the outgoing node links, propagating a trigger from the
    /// originating node.
    fn trigger_outgoing(&self) {
        for out_link in self.0.values() {
            out_link.trigger();
        }
    }

    /// Disconnect all links in this link, were `curr_node` is the source of
    /// the links.
    ///
    /// This allows for two-phase disconnection from a node, avoiding the
    /// possibility of deadlock.
    fn disconnect(self, curr_node: NonZero<usize>) {
        // For each outgoing link, we need to remove the corresponding incoming link
        // in the connected node.
        for (_, link) in self.0 {
            link.drop_node(curr_node);
        }
    }
}

struct NodeInner {
    resolved: Option<NodeState>,
    input_count: usize,
    output_count: usize,
    links: EdgeSet,
}

pub(super) struct NodeRef<P: RefPolicy> {
    inner: Arc<Mutex<NodeInner>>,
    policy: P,
}

impl<P> NodeRef<P>
where
    P: RefPolicy,
{
    fn remove_edge(&self, link: NonZero<usize>) {
        let mut inner = self.inner.lock().unwrap();
        inner.links.0.remove(&EdgeKey(link));
    }

    fn add_edge(&self, edge: Edge) -> bool {
        let addr = edge.id();
        let mut inner = self.inner.lock().unwrap();
        if let Some(state) = inner.resolved {
            // When adding an outgoing edge when triggered, we propagate the
            // trigger immediately.
            if let Edge::OutgoingNode(out_node) = edge
                && let NodeState::Triggered = state
            {
                drop(inner);
                out_node.trigger();
            }
            return false;
        }
        match inner.links.0.entry(EdgeKey(addr)) {
            btree_map::Entry::Vacant(vac) => {
                vac.insert(edge);
            }
            btree_map::Entry::Occupied(occ) => {
                // If we have the same key, it should be the same actual
                // node. Check that the type is the same (otherwise we're
                // making an immediate loop, or are simply incoherent).
                match (occ.get(), edge) {
                    (Edge::OutgoingNode(_), Edge::OutgoingNode(_))
                    | (Edge::IncomingNode(_), Edge::IncomingNode(_))
                    | (Edge::Token(_), Edge::Token(_))
                    | (Edge::WaitSet(_), Edge::WaitSet(_)) => (),
                    _ => panic!(
                        "Cycle or conflicting edge: node already connected in the opposite direction"
                    ),
                }
            }
        }
        // We return true here, indicating that the edge is present in the
        // current node's EdgeSet. This may cause a redundant add, but this
        // should be relatively cheap.
        true
    }

    pub(super) fn state(&self) -> Option<NodeState> {
        self.inner.lock().unwrap().resolved
    }

    pub(super) fn trigger(&self) -> bool {
        let links = {
            let mut inner = self.inner.lock().unwrap();
            if let Some(resolved) = inner.resolved {
                return match resolved {
                    NodeState::Dropped => false,
                    NodeState::Triggered => true,
                };
            }

            inner.resolved = Some(NodeState::Triggered);

            // Since nodes can't be triggered more than once, we can take all of the
            // outgoing links and drop them without needing to worry about future
            // tracking.
            std::mem::take(&mut inner.links)
        };

        links.trigger_outgoing();
        links.disconnect(self.id());

        true
    }

    pub(super) fn add_token(&self, token: TaskWaker) {
        self.add_edge(Edge::Token(token));
    }

    pub(super) fn remove_token(&self, token: NonZero<usize>) {
        self.remove_edge(token);
    }

    fn id(&self) -> NonZero<usize> {
        NonZero::new(Arc::as_ptr(&self.inner).addr()).unwrap()
    }
}

impl<P> Clone for NodeRef<P>
where
    P: RefPolicy + Clone,
{
    fn clone(&self) -> Self {
        {
            let mut inner = self.inner.lock().unwrap();
            if self.policy.is_input() {
                inner.input_count += 1;
            }

            if self.policy.is_output() {
                inner.output_count += 1;
            }
        }
        NodeRef {
            inner: self.inner.clone(),
            policy: self.policy.clone(),
        }
    }
}

impl<P> Drop for NodeRef<P>
where
    P: RefPolicy,
{
    fn drop(&mut self) {
        let links = {
            let mut inner = self.inner.lock().unwrap();
            let input_count_expired = if self.policy.is_input() {
                inner.input_count -= 1;
                inner.input_count == 0
            } else {
                false
            };

            let output_count_expired = if self.policy.is_output() {
                inner.output_count -= 1;
                inner.output_count == 0
            } else {
                false
            };

            if !input_count_expired && !output_count_expired {
                return;
            }
            if inner.resolved.is_none() {
                inner.resolved = Some(NodeState::Dropped);
            }

            std::mem::take(&mut inner.links)
        };

        links.disconnect(self.id());
    }
}

impl NodeRef<OutputPolicy> {
    pub(super) fn new_resolved(state: NodeState) -> Self {
        let inner = Arc::new(Mutex::new(NodeInner {
            resolved: Some(state),
            input_count: 0,
            output_count: 1,
            links: EdgeSet::new(),
        }));
        NodeRef {
            inner,
            policy: OutputPolicy,
        }
    }

    pub(crate) fn connect(self, input: NodeRef<InputPolicy>) {
        if let Some(state) = self.state() {
            // Our own node has been resolved, so we won't need to create a
            // real connection.
            match state {
                NodeState::Dropped => {
                    // Our node has been dropped. Since this is an output,
                    // it means that all of our inputs have been dropped. We
                    // can never contribute to trigger propagation, so we
                    // shouldn't do anything.
                }
                NodeState::Triggered => {
                    // Our node has been triggered. Assuming that orders of
                    // triggering and connecting should result in the same
                    // result, this means that we should propagate our current
                    // trigger to the input.
                    // Performing this connect should propagate the triggering to the
                    // next node, after which the connection is no longer necessary.
                    input.trigger();
                }
            }
            // Whatever happened, our node will not change in a way that can
            // affect input, so we can just drop ourselves.
            return;
        }

        if input.state().is_some() {
            // The target has been resolved, so this link cannot contribute to
            // its resolution. We can just drop everything.
            return;
        }

        if !input.add_edge(Edge::IncomingNode(self.clone())) {
            return;
        }

        if !self.add_edge(Edge::OutgoingNode(input.clone())) {
            input.remove_edge(self.id());
        }
    }
}

pub(super) fn create_node() -> (NodeRef<InputPolicy>, NodeRef<OutputPolicy>) {
    let inner = Arc::new(Mutex::new(NodeInner {
        resolved: None,
        input_count: 1,
        output_count: 1,
        links: EdgeSet::new(),
    }));
    (
        NodeRef {
            inner: inner.clone(),
            policy: InputPolicy,
        },
        NodeRef {
            inner,
            policy: OutputPolicy,
        },
    )
}

/// The type of ref a given NodeRef is.
///
/// Refs can count as input refs, output refs, both or neither.
pub(super) trait RefPolicy: Clone {
    fn is_input(&self) -> bool;
    fn is_output(&self) -> bool;
}

#[derive(Clone)]
pub(super) struct InputPolicy;

impl RefPolicy for InputPolicy {
    fn is_input(&self) -> bool {
        true
    }

    fn is_output(&self) -> bool {
        false
    }
}

#[derive(Clone)]
pub(super) struct OutputPolicy;

impl RefPolicy for OutputPolicy {
    fn is_input(&self) -> bool {
        false
    }

    fn is_output(&self) -> bool {
        true
    }
}

struct WaitSetInner {
    input_count: usize,
    next_ord: NonZero<usize>,
    id_to_ord: HashMap<NonZero<usize>, NonZero<usize>>,
    ord_to_triggers: BTreeMap<NonZero<usize>, NodeRef<InputPolicy>>,
}

impl WaitSetInner {
    /// Adds the trigger into the wait set.
    ///
    /// Returns true iff the trigger didn't previously exist in the wait set.
    fn add_trigger(&mut self, trigger: NodeRef<InputPolicy>) -> bool {
        let hash_map::Entry::Vacant(vac) = self.id_to_ord.entry(trigger.id()) else {
            // A node can't be added more than once to a WaitSet, as once it's resolved
            // it won't be able to observe another trigger from WaitSet.
            return false;
        };
        let ord = self.next_ord;
        vac.insert(ord);
        self.next_ord = self.next_ord.checked_add(1).unwrap();
        self.ord_to_triggers.insert(ord, trigger.clone());
        true
    }

    fn remove_trigger(&mut self, id: NonZero<usize>) -> Option<NodeRef<InputPolicy>> {
        let ord = self.id_to_ord.remove(&id)?;
        self.ord_to_triggers.remove(&ord)
    }

    fn remove_next(&mut self) -> Option<NodeRef<InputPolicy>> {
        let (_, trigger) = self.ord_to_triggers.pop_first()?;
        self.id_to_ord.remove(&trigger.id());
        Some(trigger)
    }

    fn remove_all(&mut self) -> Vec<NodeRef<InputPolicy>> {
        self.id_to_ord.clear();
        std::mem::take(&mut self.ord_to_triggers)
            .into_values()
            .collect()
    }
}

pub(super) struct WaitSetRef<P>
where
    P: RefPolicy,
{
    inner: Arc<Mutex<WaitSetInner>>,
    policy: P,
}

impl<P> WaitSetRef<P>
where
    P: RefPolicy,
{
    fn new_with_policy(policy: P, inner: Arc<Mutex<WaitSetInner>>) -> Self {
        {
            let mut inner = inner.lock().unwrap();
            if policy.is_input() {
                inner.input_count += 1;
            }
            // We don't keep track of outputs, as a WaitSet remains useful
            // even after all outputs are dropped
        }
        WaitSetRef { inner, policy }
    }

    fn get_output_ref(&self) -> WaitSetRef<OutputPolicy> {
        WaitSetRef::new_with_policy(OutputPolicy, self.inner.clone())
    }

    pub(super) fn id(&self) -> NonZero<usize> {
        NonZero::new(Arc::as_ptr(&self.inner).addr()).unwrap()
    }

    pub(super) fn remove_edge(&self, id: NonZero<usize>) {
        self.inner.lock().unwrap().remove_trigger(id);
    }
}

impl WaitSetRef<InputPolicy> {
    pub(super) fn new() -> Self {
        let inner = Arc::new(Mutex::new(WaitSetInner {
            input_count: 0,
            next_ord: NonZero::new(1).unwrap(),
            id_to_ord: HashMap::new(),
            ord_to_triggers: BTreeMap::new(),
        }));

        Self::new_with_policy(InputPolicy, inner)
    }

    pub(super) fn add_node(&self, node: &NodeRef<InputPolicy>) {
        {
            let mut inner = self.inner.lock().unwrap();
            if !inner.add_trigger(node.clone()) {
                return;
            }
        }

        if !node.add_edge(Edge::WaitSet(self.get_output_ref())) {
            // The edge is not in the node for whatever reason, so we need to
            // remove ourselves from the wait set.
            self.remove_edge(node.id());
        }
    }

    /// Tries to trigger the first of the connected nodes. Returns true if a node
    /// was triggered.
    pub(super) fn trigger_next(&self) -> bool {
        loop {
            let Some(trigger) = self.inner.lock().unwrap().remove_next() else {
                return false;
            };
            // This should always cause all of the edges (including the one pointing
            // to ourselves) to be dropped, so no need to explicitly remove
            // ourselves here.
            //
            // If this returns false, then we didn't actually cause a trigger, so we
            // loop to the next one.
            if trigger.trigger() {
                return true;
            }
        }
    }

    pub(super) fn trigger_all(&self) -> usize {
        let triggers = self.inner.lock().unwrap().remove_all();

        let mut num_triggers = 0;

        for trigger in triggers {
            if trigger.trigger() {
                num_triggers += 1;
            }
        }

        num_triggers
    }
}

impl<P> Clone for WaitSetRef<P>
where
    P: RefPolicy,
{
    fn clone(&self) -> Self {
        Self::new_with_policy(self.policy.clone(), self.inner.clone())
    }
}

impl<P> Drop for WaitSetRef<P>
where
    P: RefPolicy,
{
    fn drop(&mut self) {
        let ord_to_triggers = {
            let mut inner = self.inner.lock().unwrap();
            let input_count_expired = if self.policy.is_input() {
                inner.input_count -= 1;
                inner.input_count == 0
            } else {
                false
            };
            if !input_count_expired {
                return;
            }
            std::mem::take(&mut inner.ord_to_triggers)
        };

        // Disconnect all contained nodes. We don't trigger it, as that can
        // only be done via the trigger_one or other methods.
        for node in ord_to_triggers.values() {
            node.remove_edge(self.id());
        }
    }
}

pub(super) fn create_wait_set() -> (WaitSetRef<InputPolicy>, WaitSetRef<OutputPolicy>) {
    let inner = Arc::new(Mutex::new(WaitSetInner {
        input_count: 0,
        next_ord: NonZero::new(1).unwrap(),
        id_to_ord: HashMap::new(),
        ord_to_triggers: BTreeMap::new(),
    }));

    (
        WaitSetRef::new_with_policy(InputPolicy, inner.clone()),
        WaitSetRef::new_with_policy(OutputPolicy, inner),
    )
}
