use std::hash::{Hash, Hasher};

use crate::{Artifact, ArtifactKey, Pending, TaskContext};

pub trait Op<M1, M2> {
    type Value: Artifact;
    fn make_pending<'a>(self, ctx: &TaskContext<'a>) -> Pending<'a, &'a Self::Value>;
}

pub trait SyncOp: Hash + Eq + ArtifactKey {
    fn run(&self, ctx: &TaskContext<'_>) -> Self::Value;
}

pub trait AsyncOp: Hash + Eq + ArtifactKey {
    fn run(&self, ctx: &TaskContext<'_>) -> impl Future<Output = Self::Value> + Send;
}

/// Helper for statically checking if a type is ZST.
pub trait ZeroSized {
    const IS_ZST: bool;
}

impl<T> ZeroSized for T {
    const IS_ZST: bool = std::mem::size_of::<T>() == 0;
}

#[derive(Clone, Copy)]
pub struct FnOp<F>(F);

impl<F, R> FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + ZeroSized + Copy + Send + Sync + 'static,
    R: Artifact,
{
    pub const fn new(f: F) -> Self {
        Self(f)
    }
}

impl<F, R> Hash for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + ZeroSized + Copy + Artifact,
    R: Artifact,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&[]);
    }
}

impl<F, R> Eq for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + ZeroSized + Copy + Artifact,
    R: Artifact,
{
}

impl<F, R> PartialEq for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + ZeroSized + Copy + Artifact,
    R: Artifact,
{
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<F, R> ArtifactKey for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + ZeroSized + Copy + Send + Sync + 'static,
    R: Artifact,
{
    type Value = R;
}

impl<F, R> SyncOp for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + ZeroSized + Copy + Send + Sync + 'static,
    R: Artifact,
{
    fn run(&self, ctx: &TaskContext<'_>) -> Self::Value {
        (self.0)(ctx)
    }
}

pub trait FnExt<R>: Fn(&TaskContext<'_>) -> R + Sized + ZeroSized + Copy + Artifact
where
    R: Artifact,
{
    fn to_op(self) -> impl SyncOp<Value = R>;
}

impl<F, R> FnExt<R> for F
where
    F: Fn(&TaskContext<'_>) -> R + ZeroSized + Copy + Artifact,
    R: Artifact,
{
    fn to_op(self) -> impl SyncOp<Value = R> {
        FnOp::new(self)
    }
}

pub struct FnAsyncOp<T>(T);

impl<F, Fut> FnAsyncOp<F>
where
    F: Fn(&TaskContext<'_>) -> Fut + ZeroSized + Copy + Artifact,
    Fut: Future + Send + Sync,
    Fut::Output: Artifact,
{
    fn new(f: F) -> Self {
        Self(f)
    }
}

impl<F> Hash for FnAsyncOp<F>
where
    F: ZeroSized,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&[]);
    }
}

impl<F> Eq for FnAsyncOp<F> {}

impl<F> PartialEq for FnAsyncOp<F>
where
    F: ZeroSized,
{
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<F, Fut> ArtifactKey for FnAsyncOp<F>
where
    F: Fn(&TaskContext<'_>) -> Fut + Copy + Artifact,
    Fut: Future + Send + Sync,
    Fut::Output: Artifact,
{
    type Value = Fut::Output;
}

impl<F, Fut> AsyncOp for FnAsyncOp<F>
where
    F: Fn(&TaskContext<'_>) -> Fut + ZeroSized + Copy + Artifact,
    Fut: Future + Send + Sync,
    Fut::Output: Artifact,
{
    async fn run(&self, ctx: &TaskContext<'_>) -> Fut::Output {
        (self.0)(ctx).await
    }
}

pub struct FnMarker;
pub struct OpMarker;

/// A trait to convert an arbitrary argument into an opertation.
///
/// M is a type that is used to allow for multiple blanket implementations.
/// You should not need to specify M, and it should be left free when taken
/// as an argument. Impls should define a specific struct type.
pub trait IntoSyncOp<M>: Sized {
    type Value: Artifact;
    type Op: SyncOp<Value = Self::Value> + Artifact;

    fn into_op(self) -> Self::Op;
}

impl<F, R> IntoSyncOp<FnMarker> for F
where
    F: Fn(&TaskContext<'_>) -> R + ZeroSized + Copy + Artifact,
    R: Artifact,
{
    type Value = R;
    type Op = FnOp<Self>;

    fn into_op(self) -> Self::Op {
        FnOp::new(self)
    }
}

impl<T> IntoSyncOp<OpMarker> for T
where
    T: SyncOp,
{
    type Value = T::Value;
    type Op = T;

    fn into_op(self) -> Self::Op {
        self
    }
}

pub trait IntoAsyncOp<M>: Sized {
    type Value: Artifact;
    type Op: AsyncOp<Value = Self::Value> + Artifact;

    fn into_op(self) -> Self::Op;
}

impl<F, Fut> IntoAsyncOp<FnMarker> for F
where
    F: Fn(&TaskContext<'_>) -> Fut + ZeroSized + Copy + Artifact,
    Fut: Future + Send + Sync,
    Fut::Output: Artifact,
{
    type Value = Fut::Output;
    type Op = FnAsyncOp<Self>;

    fn into_op(self) -> Self::Op {
        FnAsyncOp::new(self)
    }
}

impl<T> IntoAsyncOp<OpMarker> for T
where
    T: AsyncOp,
{
    type Value = T::Value;
    type Op = T;

    fn into_op(self) -> Self::Op {
        self
    }
}

pub struct SyncMarker;
pub struct AsyncMarker;

impl<T, M2> Op<SyncMarker, M2> for T
where
    T: IntoSyncOp<M2>,
{
    type Value = T::Value;
    fn make_pending<'a>(self, ctx: &TaskContext<'a>) -> Pending<'a, &'a Self::Value> {
        ctx.sync_op(self.into_op())
    }
}

impl<T, M2> Op<AsyncMarker, M2> for T
where
    T: IntoAsyncOp<M2>,
{
    type Value = T::Value;
    fn make_pending<'a>(self, ctx: &TaskContext<'a>) -> Pending<'a, &'a Self::Value> {
        ctx.async_op(self.into_op())
    }
}
