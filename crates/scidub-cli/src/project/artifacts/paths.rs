use scidev_inc::{ArtifactKey, FnExt, Operation, TaskContext};
use sciproj::path::abspath::AbsPathBuf;

use crate::project::{MANIFEST_FILE_NAME, PROJECT_FILE_NAME};

#[derive(Hash, Eq, PartialEq, Debug)]
pub(super) struct ConfigPath;

impl ArtifactKey for ConfigPath {
    type Value = anyhow::Result<AbsPathBuf>;
}

impl Operation for ConfigPath {
    fn run(&self, ctx: &TaskContext<'_>) -> Self::Value {
        let root = ctx.get_input(super::root::Root);
        let path = root.join(PROJECT_FILE_NAME);
        anyhow::ensure!(path.is_file(), "Project file not found");
        Ok(path)
    }
}

pub(super) fn config_path_fn(ctx: &TaskContext<'_>) -> anyhow::Result<AbsPathBuf> {
    let root = ctx.get_input(super::root::Root);
    let path = root.join(PROJECT_FILE_NAME);
    anyhow::ensure!(path.is_file(), "Project file not found");
    Ok(path)
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub(super) struct ManifestPath;

impl ArtifactKey for ManifestPath {
    type Value = AbsPathBuf;
}

impl Operation for ManifestPath {
    fn run(&self, ctx: &TaskContext<'_>) -> Self::Value {
        let root = ctx.get_input(super::root::Root);
        root.join(MANIFEST_FILE_NAME)
    }
}
