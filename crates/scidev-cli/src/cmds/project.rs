use std::path::Path;

use sciproj::project::Project;

pub(crate) fn init_project(path: &Path) -> anyhow::Result<()> {
    let target_dir = path.canonicalize()?;

    // Some initial checks to sanity check the target directory

    println!("Initializing new project in {}", target_dir.display());
    Project::create_at(&target_dir)?;
    Ok(())
}
