use std::collections::HashMap;

use sciproj::path::relpath::{RelPath, RelPathBuf};
use serde::Deserialize;

/// Root project config schema.
#[derive(Debug, Deserialize)]
pub(crate) struct ProjectConfig {
    voice_script_config: Option<RelPathBuf>,
    game_files: Option<RelPathBuf>,
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
    pub(crate) fn voice_script_config(&self) -> Option<&RelPath> {
        self.voice_script_config.as_deref()
    }

    pub(crate) fn game_files(&self) -> Option<&RelPath> {
        self.game_files.as_deref()
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
