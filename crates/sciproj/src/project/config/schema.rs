use serde::{Deserialize, Serialize};

use crate::project::file_mapping::MappingRuleSpec;

fn default_sci_version() -> String {
    "1.1-late".to_string()
}

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct RootMappingConfig {
    #[serde(default)]
    rules: Vec<MappingRuleSpec>,
    #[serde(default)]
    excludes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub(super) struct ProjectConfig {
    /// The SCI version that will be generated for this project.
    #[serde(default = "default_sci_version")]
    sci_version: String,

    /// Configuration about the base game to use, if this is intended to be
    /// a mod/patch on an existing game.
    #[serde(default)]
    base_game: Option<BaseGame>,

    /// Define the mapping rules for the files in this project.
    mappings: RootMappingConfig,
}

#[derive(Serialize, Deserialize, Debug)]
struct BaseGame {
    /// The path to the root of the base game installation.
    ///
    /// This must be a relative path without "." or ".." components relative
    /// to the project root. This is not intended to be committed to
    /// version control, but rather set up per-developer. On first import,
    /// hashes of the base game files will be computed and stored in the
    /// project's state file.
    #[serde(rename = "root-path")]
    root_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct DirConfig {
    mapping_rules: Vec<MappingRuleSpec>,
}
