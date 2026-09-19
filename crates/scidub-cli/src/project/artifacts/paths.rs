use scidev_inc::TaskContext;
use sciproj::path::abspath::AbsPathBuf;

use crate::project::{MANIFEST_FILE_NAME, PROJECT_FILE_NAME};

pub(super) fn config_path(ctx: &TaskContext<'_>) -> anyhow::Result<AbsPathBuf> {
    let root = ctx.get_input(super::root::Root);
    let path = root.join(PROJECT_FILE_NAME);
    anyhow::ensure!(path.is_file(), "Project file not found");
    Ok(path)
}

pub(super) fn manifest_path(ctx: &TaskContext<'_>) -> AbsPathBuf {
    let root = ctx.get_input(super::root::Root);
    root.join(MANIFEST_FILE_NAME)
}
