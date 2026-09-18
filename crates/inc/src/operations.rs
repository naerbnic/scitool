use std::hash::{Hash, Hasher};

use crate::{Artifact, ArtifactKey, TaskContext};

pub trait Operation: Hash + Eq + ArtifactKey {
    fn run(&self, ctx: &TaskContext<'_>) -> Self::Value;
}

pub trait AsyncOperation: Hash + Eq + ArtifactKey {
    fn run(&self, ctx: &TaskContext<'_>) -> impl Future<Output = Self::Value> + Send;
}

#[derive(Clone, Copy)]
pub struct FnOp<F>(F);

impl<F, R> FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + Copy + Send + Sync + 'static,
    R: Artifact,
{
    pub const fn new(f: F) -> Self {
        Self(f)
    }
}

impl<F, R> Hash for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + Copy + Send + Sync + 'static,
    R: Artifact,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        // This should be a function item with no members, thus a ZST.
        assert_eq!(std::mem::size_of::<F>(), 0);
        state.write(&[]);
    }
}

impl<F, R> Eq for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + Copy + Artifact,
    R: Artifact,
{
}

impl<F, R> PartialEq for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + Copy + Artifact,
    R: Artifact,
{
    fn eq(&self, _other: &Self) -> bool {
        assert_eq!(std::mem::size_of::<F>(), 0);
        true
    }
}

impl<F, R> ArtifactKey for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + Copy + Send + Sync + 'static,
    R: Artifact,
{
    type Value = R;
}

impl<F, R> Operation for FnOp<F>
where
    F: Fn(&TaskContext<'_>) -> R + Copy + Send + Sync + 'static,
    R: Artifact,
{
    fn run(&self, ctx: &TaskContext<'_>) -> Self::Value {
        (self.0)(ctx)
    }
}

pub trait FnExt<R>: Fn(&TaskContext<'_>) -> R + Sized + Copy + Artifact
where
    R: Artifact,
{
    fn to_op(self) -> impl Operation<Value = R>;
}

impl<F, R> FnExt<R> for F
where
    F: Fn(&TaskContext<'_>) -> R + Copy + Artifact,
    R: Artifact,
{
    fn to_op(self) -> impl Operation<Value = R> {
        FnOp::new(self)
    }
}

pub struct FnMarker;
pub struct OpMarker;

/// A trait to convert an arbitrary argument into an opertation.
///
/// M is a type that is used to allow for multiple blanket implementations.
/// You should not need to specify M, and it should be left free when taken
/// as an argument. Impls should define a specific struct type.
pub trait IntoOperation<M>: Sized {
    type Value: Artifact;
    type Op: Operation<Value = Self::Value> + Artifact;

    fn into_op(self) -> Self::Op;
}

impl<F, R> IntoOperation<FnMarker> for F
where
    F: Fn(&TaskContext<'_>) -> R + Copy + Artifact,
    R: Artifact,
{
    type Value = R;
    type Op = FnOp<Self>;

    fn into_op(self) -> Self::Op {
        FnOp::new(self)
    }
}

impl<T> IntoOperation<OpMarker> for T
where
    T: Operation,
{
    type Value = T::Value;
    type Op = T;

    fn into_op(self) -> Self::Op {
        self
    }
}
