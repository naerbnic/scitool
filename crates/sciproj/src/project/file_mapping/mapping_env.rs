use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{
    helpers::paths::is_relative_path,
    project::file_mapping::rule_set::{self, RuleSet},
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum CreateError {
    #[error("Non-simple relative path: {0}")]
    NonRelativePath(PathBuf),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ApplyError {
    #[error("Rule set error: {0}")]
    RuleSetError(#[from] rule_set::ApplyError),
}

#[derive(Debug)]
pub(crate) struct MappingEnv {
    rule_sets: BTreeMap<PathBuf, RuleSet>,
}

impl MappingEnv {
    pub(crate) fn new(
        rule_sets: impl IntoIterator<Item = impl Into<(PathBuf, RuleSet)>>,
    ) -> Result<Self, CreateError> {
        let rule_sets: BTreeMap<PathBuf, RuleSet> = rule_sets.into_iter().map(Into::into).collect();

        for path in rule_sets.keys() {
            if !is_relative_path(path) {
                return Err(CreateError::NonRelativePath(path.clone()));
            }
        }

        Ok(Self { rule_sets })
    }

    pub(crate) fn apply(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<Option<BTreeMap<String, String>>, ApplyError> {
        let path = path.as_ref();

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
