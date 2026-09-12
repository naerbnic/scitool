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
    script_url: Option<String>,
    #[serde(default)]
    source: SourceConfig,
    #[serde(default)]
    targets: HashMap<String, Target>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Target {
    line_mapping: RelPathBuf,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct SourceConfig {
    /// The directory that contains all source code. This is a path that is
    /// relative to the root of the project.
    ///
    /// If unset, this defaults to the `scripts` directory under the root.
    source_dir: Option<RelPathBuf>,

    /// A list of files that will be unconditionally included in every
    /// source file.
    ///
    /// Paths are provided relative to the source directory.
    #[serde(default)]
    global_includes: Vec<RelPathBuf>,

    /// A list of include paths to use within the project.
    ///
    /// All paths are relative to the source directory, and must be
    /// contained inside the directory.
    ///
    /// If not set, this defaults to the root of the source directory. If a
    /// list is provided, that list will be used verbatim.
    include_paths: Option<Vec<RelPathBuf>>,
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

    pub(crate) fn script_url(&self) -> Option<&str> {
        self.script_url.as_deref()
    }

    pub(crate) fn targets(&self) -> &HashMap<String, Target> {
        &self.targets
    }

    pub(crate) fn source_config(&self) -> &SourceConfig {
        &self.source
    }
}

impl Target {
    pub(crate) fn line_mapping(&self) -> &RelPath {
        &self.line_mapping
    }
}

impl SourceConfig {
    pub(crate) fn source_dir(&self) -> Option<&RelPath> {
        self.source_dir.as_deref()
    }

    pub(crate) fn global_includes(&self) -> Vec<&RelPath> {
        self.global_includes.iter().map(|p| &**p).collect()
    }

    pub(crate) fn include_paths(&self) -> Option<Vec<&RelPath>> {
        self.include_paths
            .as_ref()
            .map(|inc_paths| inc_paths.iter().map(|p| &**p).collect())
    }
}
