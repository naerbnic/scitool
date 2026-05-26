use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Context;
use sciproj::path::relpath::{RelPath, RelPathBuf};
use serde::Deserialize;

/// Root project config schema.
#[derive(Debug, Deserialize)]
pub(crate) struct ProjectConfig {
    voice_script_config: RelPathBuf,
    game_files: RelPathBuf,
    audio_files_root: Option<RelPathBuf>,
    build_dir: Option<RelPathBuf>,
    #[serde(default)]
    targets: HashMap<String, Target>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Target {
    line_mapping: RelPathBuf,
}

impl ProjectConfig {
    pub(crate) fn voice_script_config(&self) -> &RelPath {
        &self.voice_script_config
    }

    pub(crate) fn game_files(&self) -> &RelPath {
        &self.game_files
    }

    pub(crate) fn audio_files_root(&self) -> Option<&RelPath> {
        self.audio_files_root.as_deref()
    }

    pub(crate) fn build_dir(&self) -> Option<&RelPath> {
        self.build_dir.as_deref()
    }

    pub(crate) fn targets(&self) -> &HashMap<String, Target> {
        &self.targets
    }
}

impl Target {
    pub(crate) fn line_mapping(&self) -> &RelPath {
        &self.line_mapping
    }
}

const PROJECT_FILE_NAME: &str = "scidub.toml";

pub(crate) fn find_project_root(start_path: &impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let canon_path = start_path.as_ref().canonicalize()?;
    let mut current_path: &Path = &canon_path;
    anyhow::ensure!(
        current_path.exists(),
        "Can't find project from nonexistent path: {}",
        current_path.display()
    );
    loop {
        let possible_project_file = current_path.join(PROJECT_FILE_NAME);
        if possible_project_file.is_file() {
            return Ok(current_path.to_path_buf());
        }
        current_path = current_path.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "Reached root directory without finding project file {PROJECT_FILE_NAME}"
            )
        })?;
    }
}

pub(crate) fn load_project(root: &Path) -> anyhow::Result<ProjectConfig> {
    let project_file = root.join(PROJECT_FILE_NAME);
    let project: ProjectConfig = toml::from_slice(&std::fs::read(&project_file).context(
        anyhow::format_err!("Unable to open project file {}", project_file.display()),
    )?)
    .context(format!(
        "Unable to parse project file {}",
        project_file.display()
    ))?;
    Ok(project)
}
