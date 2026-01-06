use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use crate::project::{
    file_mapping::rule_set::{self, RuleSet},
    paths::{MatchError, PathMatcher, Pattern},
};

fn is_simple_relative_path(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("Non-simple relative path: {0}")]
    NonRelativePath(PathBuf),

    #[error("Placeholder in exclude pattern")]
    PlaceholdersInExcludes,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("Rule set error: {0}")]
    RuleSetError(#[from] rule_set::ApplyError),

    #[error("Error in exclude matcher: {0}")]
    Exclude(#[from] MatchError),
}

#[derive(Debug)]
pub struct MappingEnv {
    rule_sets: BTreeMap<PathBuf, RuleSet>,
    excludes: Vec<PathMatcher>,
}

impl MappingEnv {
    pub fn new(
        rule_sets: impl IntoIterator<Item = impl Into<(PathBuf, RuleSet)>>,
        excludes: impl IntoIterator<Item = impl Into<PathMatcher>>,
    ) -> Result<Self, CreateError> {
        let rule_sets: BTreeMap<PathBuf, RuleSet> = rule_sets.into_iter().map(Into::into).collect();
        let excludes: Vec<_> = excludes.into_iter().map(Into::into).collect();

        for path in rule_sets.keys() {
            if !is_simple_relative_path(path) {
                return Err(CreateError::NonRelativePath(path.clone()));
            }
        }

        if excludes.iter().any(|m| !m.placeholders().is_empty()) {
            return Err(CreateError::PlaceholdersInExcludes);
        }

        Ok(Self {
            rule_sets,
            excludes,
        })
    }

    pub fn apply(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<Option<BTreeMap<String, String>>, ApplyError> {
        let path = path.as_ref();

        for exclude_matcher in &self.excludes {
            if exclude_matcher.match_path(path)?.is_some() {
                return Ok(None);
            }
        }

        let mut rules_sets = Vec::new();
        for ancestor in path.ancestors() {
            if let Some(rule_set) = self.rule_sets.get(ancestor) {
                rules_sets.push((ancestor, rule_set));
            }
        }

        // rules_sets should now be in the order of longest path first, which is
        // the order we want to apply them in.

        let mut found_match = false;

        let mut prop_map = BTreeMap::new();
        for (base_dir, rule_set) in rules_sets {
            // Each rule set is relative to the base_dir it is defined in.
            assert!(path.starts_with(base_dir));
            let relative_path = path
                .strip_prefix(base_dir)
                .expect("Path was generated from ancestors()");
            let matched = rule_set.apply(relative_path, &mut prop_map)?;
            found_match |= matched;
        }

        Ok(if found_match { Some(prop_map) } else { None })
    }
}
