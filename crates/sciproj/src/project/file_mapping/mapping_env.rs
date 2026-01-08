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

#[cfg(test)]
mod tests {
    use crate::{
        helpers::test::{from_json, make_map},
        project::file_mapping::MappingRuleSpec,
    };

    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(transparent)]
    struct RuleSetSpec(BTreeMap<PathBuf, Vec<MappingRuleSpec>>);

    fn make_mapping_env(spec: RuleSetSpec) -> anyhow::Result<MappingEnv> {
        let rule_sets = spec
            .0
            .into_iter()
            .map(|(path, rules)| Ok::<_, anyhow::Error>((path, RuleSet::from_spec(&rules)?)))
            .collect::<anyhow::Result<BTreeMap<PathBuf, RuleSet>>>()?;
        Ok(MappingEnv::new(rule_sets)?)
    }

    #[test]
    fn test_apply() -> anyhow::Result<()> {
        let env = make_mapping_env(from_json!({
            "": [
                {
                    "includes": ["**/*.rs"],
                    "properties": {
                        "lang": "rust"
                    }
                },
                {
                    "includes": ["**/*.py"],
                    "properties": {
                        "lang": "python"
                    }
                }
            ]
        }))?;

        let props = env.apply("src/main.rs")?;
        assert_eq!(props, Some(make_map([("lang", "rust")])));

        let props = env.apply("src/main.py")?;
        assert_eq!(props, Some(make_map([("lang", "python")])));

        let props = env.apply("src/main.c")?;
        assert_eq!(props, None);

        Ok(())
    }

    #[test]
    fn test_nested_override() -> anyhow::Result<()> {
        // A nested rule overrides the outer rule.
        let env = make_mapping_env(from_json!({
            "": [
                {
                    "includes": ["**/*.rs"],
                    "properties": {
                        "lang": "rust"
                    }
                },
                {
                    "includes": ["**/*.py"],
                    "properties": {
                        "lang": "python"
                    }
                }
            ],
            "docs": [
                {
                    "includes": ["**/*.rs"],
                    "properties": {
                        "lang": "some-other-type"
                    }
                }
            ]
        }))?;

        let props = env.apply("src/main.rs")?;
        assert_eq!(props, Some(make_map([("lang", "rust")])));

        let props = env.apply("docs/main.rs")?;
        assert_eq!(props, Some(make_map([("lang", "some-other-type")])));

        Ok(())
    }

    #[test]
    fn test_disjoint() -> anyhow::Result<()> {
        // Disjoint property sets merge
        let env = make_mapping_env(from_json!({
            "": [
                {
                    "includes": ["**/*.rs"],
                    "properties": {
                        "lang": "rust"
                    }
                },
                {
                    "includes": ["**/*.md"],
                    "properties": {
                        "lang": "markdown"
                    }
                }
            ],
            "docs": [
                {
                    "includes": ["**"],
                    "properties": {
                        "category": "docs"
                    }
                }
            ],
            "src": [
                {
                    "includes": ["**"],
                    "properties": {
                        "category": "source"
                    }
                }
            ]
        }))?;

        assert_eq!(
            env.apply("src/main.rs")?,
            Some(make_map([("lang", "rust"), ("category", "source")]))
        );

        assert_eq!(
            env.apply("docs/main.md")?,
            Some(make_map([("lang", "markdown"), ("category", "docs")]))
        );

        Ok(())
    }

    #[test]
    fn test_partial_inner_overrides() -> anyhow::Result<()> {
        let env = make_mapping_env(from_json!({
            "": [
                {
                    "includes": ["**/*.rs"],
                    "properties": {
                        "lang": "rust",
                        "category": "unknown"
                    }
                },
            ],
            "docs": [
                {
                    "includes": ["**"],
                    "properties": {
                        "category": "docs"
                    }
                }
            ],
            "src": [
                {
                    "includes": ["**"],
                    "properties": {
                        "category": "source"
                    }
                }
            ]
        }))?;

        assert_eq!(
            env.apply("tools/main.rs")?,
            Some(make_map([("lang", "rust"), ("category", "unknown")]))
        );

        assert_eq!(
            env.apply("docs/main.rs")?,
            Some(make_map([("lang", "rust"), ("category", "docs")]))
        );

        assert_eq!(
            env.apply("src/main.rs")?,
            Some(make_map([("lang", "rust"), ("category", "source")]))
        );

        Ok(())
    }
}
