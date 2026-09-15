use std::hash::Hash;

use crate::{ArtifactKey, TaskContext};

pub trait Operation: Hash + Eq + ArtifactKey {
    fn run(&self, ctx: &TaskContext<'_>) -> Self::Value;
}

pub trait AsyncOperation: Hash + Eq + ArtifactKey {
    fn run(&self, ctx: &TaskContext<'_>) -> impl Future<Output = Self::Value> + Send;
}
