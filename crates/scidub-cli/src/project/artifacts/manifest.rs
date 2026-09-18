use anyhow::Context as _;
use scidev_inc::{ArtifactKey, Operation, TaskContext};
use sciproj::path::abspath::AbsPathBuf;

use crate::project::{MANIFEST_FILE_NAME, Manifest};

#[derive(Hash, Eq, PartialEq, Debug)]
pub(super) struct ManifestOpt;

impl ArtifactKey for ManifestOpt {
    type Value = anyhow::Result<Option<Manifest>>;
}

impl Operation for ManifestOpt {
    fn run(&self, ctx: &TaskContext<'_>) -> Self::Value {
        let manifest_path = ctx.get_input(super::root::Root).join(MANIFEST_FILE_NAME);
        if !manifest_path.is_file() {
            return Ok(None);
        }
        let manifest: Manifest = serde_json::from_reader(std::fs::File::open(&manifest_path)?)
            .context(format!("Unable to parse manifest file {manifest_path}"))?;
        Ok(Some(manifest))
    }
}
