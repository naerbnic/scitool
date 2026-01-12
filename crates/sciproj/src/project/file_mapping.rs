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

use crate::{
    helpers::{
        iter::IterExt as _,
        paths::{DirInfo, FileLister},
    },
    project::{
        file_mapping::{mapping_env::MappingEnv, rule::MappingError, rule_set::RuleSet},
        paths::{self, PathMatcher, Pattern},
    },
};

pub(crate) use rule::MappingRuleSpec;

#[derive(Debug, thiserror::Error)]
pub(crate) enum SpecError {
    #[error(transparent)]
    Pattern(#[from] paths::ParseError),

    #[error("Excludes not able to be merged: {0}")]
    ExcludeMergeError(#[from] paths::MergeError),

    #[error("Invalid rule spec: {0}")]
    InvalidRuleSpec(#[from] rule_set::SpecError),

    #[error(transparent)]
    MappingEnv(#[from] mapping_env::CreateError),

    #[error("Placeholders not allowed in exclude patterns")]
    PlaceholdersInExcludes,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Mapping(#[from] MappingError),
    #[error(transparent)]
    EnvApply(#[from] mapping_env::ApplyError),
}

pub(crate) struct FileMapper {
    mapping_env: MappingEnv,
    excludes: PathMatcher,
}

impl FileMapper {
    #[expect(single_use_lifetimes, reason = "compile error without lifetime")]
    #[cfg_attr(not(test), expect(dead_code, reason = "in progress"))]
    #[cfg_attr(test, expect(dead_code, reason = "in progress"))]
    pub(crate) fn from_config<'a>(
        rule_sets: impl IntoIterator<
            Item = (
                impl AsRef<Path>,
                impl IntoIterator<Item = impl Into<&'a MappingRuleSpec>>,
            ),
        >,
        excludes: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, SpecError> {
        let rule_sets = rule_sets
            .into_iter()
            .map(|(path, rules)| {
                let path = path.as_ref().to_path_buf();
                let rule_set = RuleSet::from_spec(rules.into_iter().map(Into::into))?;
                Ok::<_, SpecError>((path, rule_set))
            })
            .extract_err()?;

        let mapping_env = MappingEnv::new(rule_sets)?;
        let excludes = excludes
            .into_iter()
            .map(|s| s.as_ref().parse::<Pattern>())
            .map_err(SpecError::Pattern)
            .reduce_result(|a, b| a.merge(b).map_err(SpecError::ExcludeMergeError))?
            .unwrap_or_else(Pattern::empty)
            .build_matcher();

        if excludes.placeholders().is_empty() {
            return Err(SpecError::PlaceholdersInExcludes);
        }

        Ok(Self {
            mapping_env,
            excludes,
        })
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn map_workspace_files(
        &self,
        workspace_root: impl AsRef<Path>,
    ) -> Result<FileCollection, Error> {
        let file_filter = |dir: &DirInfo| {
            self.excludes
                .match_path(dir.path())
                .map(|r| r.is_some())
                .map_err(io::Error::other)
        };

        let file_list = FileLister::new(workspace_root)
            .set_dir_filter(file_filter)
            .list_all()
            .map_err(Error::Io)?;

        // Attempt to map each file to its respective rule sets.

        let mut entries = BTreeSet::new();
        for file in file_list {
            if let Some(mappings) = self.mapping_env.apply(&file)? {
                entries.insert(FileEntry::new(file, mappings));
            }
        }

        Ok(FileCollection { entries })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FileEntry {
    path: PathBuf,
    properties: BTreeMap<String, String>,
}

impl FileEntry {
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
    entries: BTreeSet<FileEntry>,
}


