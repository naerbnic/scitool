mod artifact_store;
mod clock;
mod event;
mod executor;
mod mut_cache;
mod operations;
mod pending;
mod projected_arc;
mod promise;
mod scheduler;
mod types;
mod universe;
mod untyped_key;

pub use operations::{AsyncMarker, AsyncOp, FnAsyncOp, FnExt, FnOp, Op, SyncMarker, SyncOp};
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
    struct SyncDoubleOp;

    impl ArtifactKey for SyncDoubleOp {
        type Value = i32;
    }

    impl AsyncOp for SyncDoubleOp {
        #[allow(clippy::unused_async_trait_impl)]
        async fn run(&self, ctx: &TaskContext<'_>) -> Self::Value {
            *ctx.get_input(IntInput) * 2
        }
    }
    #[derive(Hash, Eq, PartialEq)]
    struct AsyncDoubleOp;

    impl ArtifactKey for AsyncDoubleOp {
        type Value = i32;
    }

    impl SyncOp for AsyncDoubleOp {
        fn run(&self, ctx: &TaskContext<'_>) -> Self::Value {
            *ctx.get_input(IntInput) * 2
        }
    }

    #[test]
    fn test_basic_use() {
        let mut universe = Universe::new();
        universe.add_input(IntInput, 12);

        let result = universe.op(SyncDoubleOp).get_sync();
        assert_eq!(*result, 24);
    }

    #[test]
    fn test_async_use() {
        let mut universe = Universe::new();
        universe.add_input(IntInput, 12);

        let result = block_on(universe.op(AsyncDoubleOp).get_async());
        assert_eq!(*result, 24);
    }
}
