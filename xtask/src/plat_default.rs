use directories::ProjectDirs;
use std::{path::Path, process::ExitCode};

pub(super) fn run_setup(_base_dirs: &ProjectDirs, _workspace_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

pub(super) fn run_env(
    _workspace_path: &Path,
    _cmd: &str,
    _args: &[String],
) -> anyhow::Result<ExitCode> {
    panic!("Running in the environment is unsupported")
}
