mod artifact_store;
mod clock;
mod event;
mod executor;
mod operations;
mod pending;
mod projected_arc;
mod promise;
mod scheduler;
mod types;
mod universe;
mod untyped_key;

pub use operations::{AsyncOperation, FnAsyncOp, FnExt, FnOp, Operation};
pub use pending::Pending;
pub use universe::{Artifact, ArtifactKey, Ref, TaskContext, Universe};

#[cfg(test)]
mod tests {
    use super::*;
    use futures_executor::block_on;

    #[derive(Hash, Eq, PartialEq)]
    struct IntInput;

    impl ArtifactKey for IntInput {
        type Value = i32;
    }

    #[derive(Hash, Eq, PartialEq)]
    struct DoubleOp;

    impl ArtifactKey for DoubleOp {
        type Value = i32;
    }

    impl Operation for DoubleOp {
        fn run(&self, ctx: &TaskContext<'_>) -> Self::Value {
            *ctx.get_input(IntInput) * 2
        }
    }

    impl AsyncOperation for DoubleOp {
        #[allow(clippy::unused_async_trait_impl)]
        async fn run(&self, ctx: &TaskContext<'_>) -> Self::Value {
            *ctx.get_input(IntInput) * 2
        }
    }

    #[test]
    fn test_basic_use() {
        let mut universe = Universe::new();
        universe.add_input(IntInput, 12);

        let result = universe.sync_op(DoubleOp).get_sync();
        assert_eq!(*result.get(), 24);
    }

    #[test]
    fn test_async_use() {
        let mut universe = Universe::new();
        universe.add_input(IntInput, 12);

        let result = block_on(universe.sync_op(DoubleOp).get_async());
        assert_eq!(*result.get(), 24);
    }
}
