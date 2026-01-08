//! This library contains the necessary functionality for mapping files to
//! properties. This is used to acquire metadata about files in a project for
//! association with resources.

mod mapping_env;
mod prop_templates;
mod rule;
mod rule_set;

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
};

use walkdir::DirEntry;

use crate::project::file_mapping::{rule::MappingError, rule_set::RuleSet};

pub(crate) use rule::MappingRuleSpec;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Mapping(#[from] MappingError),
}

pub(crate) struct FileMapper {
    #[expect(dead_code, reason = "in progress")]
    rule_mappings: BTreeMap<PathBuf, RuleSet>,
    ignored_paths: BTreeSet<PathBuf>,
}

impl FileMapper {
    #[expect(dead_code, reason = "in progress")]
    #[expect(clippy::todo, reason = "in progress")]
    pub(crate) fn map_workspace_files(
        &self,
        workspace_root: impl AsRef<Path>,
    ) -> Result<FileCollection, Error> {
        let walk_dir = walkdir::WalkDir::new(workspace_root);

        // Collect all file paths that are not ignored.
        let _paths = walk_dir
            .into_iter()
            .filter_entry(|entry| !self.ignored_paths.contains(entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(io::Error::from)?
            .into_iter()
            .filter(|entry| entry.file_type().is_file())
            .map(DirEntry::into_path)
            .collect::<Vec<PathBuf>>();

        // Attempt to map each file to its respective rule sets.
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FileEntry {
    path: PathBuf,
    properties: BTreeMap<String, String>,
}

impl FileEntry {
    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn new(path: impl Into<PathBuf>, properties: BTreeMap<String, String>) -> Self {
        Self {
            path: path.into(),
            properties,
        }
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn properties(&self) -> &BTreeMap<String, String> {
        &self.properties
    }
}

pub(crate) struct FileCollection {
    #[expect(dead_code, reason = "in progress")]
    files: BTreeSet<FileEntry>,
}
