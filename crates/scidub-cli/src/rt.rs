use std::time::Duration;

/// A utility to run a single async block synchronously.
///
/// This should only be run from code that knows it's starting outside an
/// asynchronous context.
pub(crate) fn run_async<F, R>(future: F) -> anyhow::Result<R>
where
    F: Future<Output = anyhow::Result<R>>,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = rt.block_on(future);
    rt.shutdown_timeout(Duration::from_secs_f32(5.0));
    result
}
