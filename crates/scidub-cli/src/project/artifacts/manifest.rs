use anyhow::Context as _;
use scidev_inc::TaskContext;

use crate::project::{MANIFEST_FILE_NAME, Manifest};

use super::root;

pub(super) fn manifest_opt(ctx: &TaskContext<'_>) -> anyhow::Result<Option<Manifest>> {
    let manifest_path = ctx.get_input(root::Root).join(MANIFEST_FILE_NAME);
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let manifest: Manifest = serde_json::from_reader(std::fs::File::open(&manifest_path)?)
        .context(format!("Unable to parse manifest file {manifest_path}"))?;
    Ok(Some(manifest))
}
