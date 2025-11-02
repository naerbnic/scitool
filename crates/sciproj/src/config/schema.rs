use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct Contents {
    project: ProjectConfig,
}

fn default_sci_version() -> String {
    "1.1-late".to_string()
}

#[derive(Serialize, Deserialize, Debug)]
struct ProjectConfig {
    /// The SCI version that will be generated for this project.
    #[serde(rename = "sci-version", default = "default_sci_version")]
    sci_version: String,

    /// Configuration about the base game to use, if this is intended to be
    /// a mod/patch on an existing game.
    #[serde(rename = "base-game")]
    base_game: Option<BaseGame>,

    /// Paths to search for resource files when searching for files to include.
    ///
    /// By default, this is the project root itself. Project config files will
    /// not be searched.
    #[serde(rename = "asset-paths")]
    asset_paths: Vec<String>,

    /// File patterns that are used to infer resource ids when importing files.
    ///
    /// Each pattern is a glob pattern that can include named capture groups
    /// '{type}' and '{num}' to capture the resource type and number. '{ext}'
    /// can also be used to capture the file extension, and '{name}' can be
    /// used to capture the actual filename, which can be any valid string.
    /// Other characters in the pattern must match exactly.
    /// 
    /// Files that match at at least one pattern will be considered for import.
    /// If any fields are missing (for example, if the pattern does not include
    /// '{num}'), either a default value will be used, or the user will be
    /// prompted to provide the missing information.
    /// 
    /// Examples:
    ///   - Classic SCI patch file names: "{num}.{type}.{ext}"
    ///   - Prefix nameed files: "{name}.{type}.{num}.{ext}"
    #[serde(rename = "import-patterns")]
    import_patterns: Vec<String>,
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
