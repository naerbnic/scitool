use std::{
    fmt::{Debug, Display},
    hash::Hash,
    ops::Deref,
    sync::Arc,
};

use crate::{
    AsyncOp, Op,
    artifact_store::ArtifactStore,
    executor::Executor,
    operations::{IntoAsyncOp, IntoSyncOp, SyncOp},
    pending::Pending,
    projected_arc::ProjArc,
    untyped_key::UntypedKey,
};

struct Inner {
    artifacts: ArtifactStore,
    executor: Executor,
}

pub struct Universe {
    inner: Arc<Inner>,
}

impl Universe {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                artifacts: ArtifactStore::new(),
                executor: Executor::new_simple(),
            }),
        }
    }

    pub fn add_input<I: ArtifactKey>(&mut self, input: I, value: I::Value) -> &mut Self where {
        self.inner
            .artifacts
            .insert(&UntypedKey::from_arc(Arc::new(input)), value);
        self
    }

    pub fn op<'a, O, M1, M2>(&'a self, op: O) -> Pending<'a, &'a O::Value>
    where
        O: Op<M1, M2> + 'a,
    {
        let ctx = TaskContext::new(&self.inner);
        ctx.op(op)
    }
}

impl Default for Universe {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Ref<T>(ProjArc<T>);

impl<T> Ref<T> {
    pub(crate) fn from_proj_arc(proj_arc: ProjArc<T>) -> Self {
        Self(proj_arc)
    }

    #[must_use]
    pub fn get(&self) -> &T {
        &self.0
    }
}

impl<E> Ref<E>
where
    E: Display + Debug + Send + Sync + 'static,
{
    #[must_use]
    pub fn into_err(self) -> RefErr<E> {
        RefErr(self)
    }
}

impl<T, E> Ref<Result<T, E>>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    pub fn can_fail(self) -> Result<Ref<T>, Ref<E>> {
        let proj_arc = &self.0;
        match proj_arc.as_ref() {
            Ok(_) => Ok(Ref::from_proj_arc(ProjArc::map(proj_arc, |v| {
                v.as_ref().ok().unwrap()
            }))),
            Err(_) => Err(Ref::from_proj_arc(ProjArc::map(proj_arc, |v| {
                v.as_ref().err().unwrap()
            }))),
        }
    }
}

impl<T, E> Ref<Result<T, E>>
where
    T: Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
{
    pub fn can_fail_cloned(self) -> Result<Ref<T>, E> {
        let proj_arc = &self.0;
        match proj_arc.as_ref() {
            Ok(_) => Ok(Ref::from_proj_arc(ProjArc::map(proj_arc, |v| {
                v.as_ref().ok().unwrap()
            }))),
            Err(e) => Err(e.clone()),
        }
    }
}

impl<T, E> Ref<Result<T, E>>
where
    T: Send + Sync + 'static,
    E: Display + Debug + Send + Sync + 'static,
{
    pub fn can_fail_disp(self) -> Result<Ref<T>, RefErr<E>> {
        let proj_arc = &self.0;
        match proj_arc.as_ref() {
            Ok(_) => Ok(Ref::from_proj_arc(ProjArc::map(proj_arc, |v| {
                v.as_ref().ok().unwrap()
            }))),
            Err(_) => {
                Err(
                    Ref::from_proj_arc(ProjArc::map(proj_arc, |v| v.as_ref().err().unwrap()))
                        .into_err(),
                )
            }
        }
    }
}

impl<E> std::error::Error for Ref<E> where E: std::error::Error {}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Debug for Ref<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<T> Display for Ref<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub struct RefErr<E>(Ref<E>);

impl<E> RefErr<E>
where
    E: Display + Debug + Send + Sync + 'static,
{
    pub(crate) fn from_ref(r: Ref<E>) -> Self {
        Self(r)
    }

    #[must_use]
    pub fn into_ref(self) -> Ref<E> {
        self.0
    }
}

impl<E> Debug for RefErr<E>
where
    E: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<E> Display for RefErr<E>
where
    E: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<E> std::error::Error for RefErr<E> where E: Display + Debug + Send + Sync + 'static {}

/// A context passed to each operation to interact with the Universe.
///
/// This permits trying to obtain new operations within another operation.
pub struct TaskContext<'a> {
    inner: &'a Arc<Inner>,
}

impl<'a> TaskContext<'a> {
    fn new(inner: &'a Arc<Inner>) -> Self {
        Self { inner }
    }

    #[must_use]
    pub fn try_get_input<I>(&self, input: I) -> Option<&I::Value>
    where
        I: ArtifactKey,
    {
        let promise = self
            .inner
            .artifacts
            .try_get::<I::Value>(&UntypedKey::from_arc(Arc::new(input)))?;
        promise.try_get()
    }

    #[must_use]
    pub fn get_input<I>(&self, input: I) -> &I::Value
    where
        I: ArtifactKey,
    {
        self.try_get_input::<I>(input).expect("Input not found")
    }

    /// Run a simple item that will be cached. Runs immediately, and the
    /// resulting value is cached.
    ///
    /// This should generally be used for non-I/O functions that need to be
    /// able to request other operations.
    pub fn cached<O, M>(&self, op: O) -> &'a O::Value
    where
        O: IntoSyncOp<M> + 'a,
        O: Artifact,
    {
        self.sync_op(op).get_sync()
    }

    /// Run a simple item that will be cached. Runs immediately, and the
    /// resulting value is cached.
    ///
    /// This should generally be used for non-I/O functions that need to be
    /// able to request other operations.
    pub fn cached_disp<O, M, T, E>(&self, op: O) -> Result<&'a T, RefErr<E>>
    where
        O: IntoSyncOp<M, Value = Result<T, E>> + 'a,
        O: Artifact,
        T: Send + Sync + 'static,
        E: Display + Debug + Send + Sync + 'static,
    {
        self.sync_op(op).can_fail_disp().get_sync()
    }

    pub fn op<O, M1, M2>(&self, op: O) -> Pending<'a, &'a O::Value>
    where
        O: Op<M1, M2> + 'a,
    {
        op.make_pending(self)
    }

    pub(crate) fn sync_op<O, M>(&self, op: O) -> Pending<'a, &'a O::Value>
    where
        O: IntoSyncOp<M> + 'a,
    {
        let op = Arc::new(op.into_op());
        let output = self.inner.artifacts.enqueue_sync(
            &self.inner.executor,
            &UntypedKey::from_arc(op.clone()),
            {
                let inner = self.inner.clone();
                move || op.run(&TaskContext::new(&inner))
            },
        );
        Pending::new_infallible(output)
    }

    pub(crate) fn async_op<O, M>(&self, op: O) -> Pending<'a, &'a O::Value>
    where
        O: IntoAsyncOp<M> + 'a,
    {
        let op = Arc::new(op.into_op());
        let output = self.inner.artifacts.enqueue_async(
            &self.inner.executor,
            &UntypedKey::from_arc(op.clone()),
            {
                let inner = self.inner.clone();
                async move { op.run(&TaskContext::new(&inner)).await }
            },
        );
        Pending::new_infallible(output)
    }
}

pub trait Artifact: Send + Sync + 'static {}
impl<T> Artifact for T where T: Send + Sync + 'static {}

pub trait ArtifactKey: Artifact + Eq + Hash {
    type Value: Artifact;
}
