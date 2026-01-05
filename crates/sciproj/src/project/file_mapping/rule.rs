use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::project::{
    file_mapping::prop_templates::{self, ApplyError, PropTemplate},
    paths::{self, MatchError, PathMatcher, Pattern},
};

#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    #[error("Provided empty name")]
    EmptyName,

    #[error("No include patterns specified")]
    NoIncludePatterns,

    #[error("Mismatched include placeholders")]
    MismatchedIncludePlaceholders,

    #[error("Placeholder in exclude pattern")]
    PlaceholderInExcludePattern,

    #[error("Invalid pattern: {0}")]
    InvalidPattern(#[from] paths::ParseError),

    #[error("Invalid property template: {0}")]
    InvalidPropertyTemplate(#[from] prop_templates::ParseError),

    #[error("Undefined template placeholder: {0}")]
    UndefinedTemplatePlaceholder(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingRuleSpec {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    name: Option<String>,
    includes: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    excludes: Vec<String>,
    properties: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    overrides: BTreeSet<String>,
}

#[derive(Debug)]
pub struct MappingRule {
    /// The name of this rule, as used to reference it in other local rules.
    name: Option<String>,

    /// Path matchers for paths that will be included.
    ///
    /// These must all have the same placeholders
    include_matchers: Vec<PathMatcher>,

    /// Path matchers for paths that will be excluded.
    ///
    /// These must all have _no_ placeholders.
    exclude_matchers: Vec<PathMatcher>,

    /// Properties that will be generated from the include matchers.
    properties: BTreeMap<String, PropTemplate>,

    /// Rule names that this rule will override.
    overrides: BTreeSet<String>,
}

impl MappingRule {
    pub fn from_spec(spec: &MappingRuleSpec) -> Result<Self, SpecError> {
        if let Some(name) = spec.name.as_ref()
            && name.is_empty()
        {
            return Err(SpecError::EmptyName);
        }

        let name = spec.name.clone();

        let mut include_matchers = Vec::new();
        for include in &spec.includes {
            let pattern = include.parse::<Pattern>()?;
            let matcher = pattern.build_matcher();
            include_matchers.push(matcher);
        }

        let include_placeholders;

        {
            // Validate include matchers.

            let mut iter = include_matchers.iter();
            let Some(first_matcher) = iter.next() else {
                return Err(SpecError::NoIncludePatterns);
            };

            let first_placeholders: BTreeSet<_> = first_matcher.placeholders().iter().collect();

            for matcher in iter {
                let curr_placeholders: BTreeSet<_> = matcher.placeholders().iter().collect();
                if curr_placeholders != first_placeholders {
                    return Err(SpecError::MismatchedIncludePlaceholders);
                }
            }

            include_placeholders = first_placeholders;
        }

        let mut exclude_matchers = Vec::new();
        for exclude in &spec.excludes {
            let pattern = exclude.parse::<Pattern>()?;
            let matcher = pattern.build_matcher();
            if !matcher.placeholders().is_empty() {
                return Err(SpecError::PlaceholderInExcludePattern);
            }
            exclude_matchers.push(matcher);
        }

        let mut properties = BTreeMap::new();
        for (name, template) in &spec.properties {
            let template = template.parse::<PropTemplate>()?;
            for placeholder in template.placeholders() {
                if !include_placeholders.contains(placeholder) {
                    return Err(SpecError::UndefinedTemplatePlaceholder(placeholder.clone()));
                }
            }
            properties.insert(name.clone(), template);
        }

        let mut overrides = BTreeSet::new();
        for override_name in &spec.overrides {
            overrides.insert(override_name.clone());
        }

        Ok(Self {
            name,
            include_matchers,
            exclude_matchers,
            properties,
            overrides,
        })
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn overrides(&self) -> impl Iterator<Item = &str> {
        self.overrides.iter().map(|s| s.as_str())
    }

    pub fn apply_rule(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<Option<BTreeMap<String, String>>, MappingError> {
        let path = path.as_ref();
        for exclude_matcher in &self.exclude_matchers {
            if exclude_matcher.match_path(path)?.is_some() {
                return Ok(None);
            }
        }

        let mut matches = Vec::new();
        for include_matcher in &self.include_matchers {
            if let Some(m) = include_matcher.match_path(path)? {
                matches.push(m);
            }
        }

        let Some((first, rest)) = matches.split_first() else {
            return Ok(None);
        };

        // Check that all matches have the same properties
        for m in rest {
            if m.properties() != first.properties() {
                return Err(MappingError::AmbiguousMatch);
            }
        }

        let mut properties = BTreeMap::new();
        for (name, template) in &self.properties {
            properties.insert(name.clone(), template.apply(first.properties())?);
        }
        Ok(Some(properties))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MappingError {
    #[error("Error during match: {0}")]
    MatchError(#[from] MatchError),

    #[error("Error applying placeholders to templates: {0}")]
    ApplyError(#[from] ApplyError),

    #[error("Ambiguous matches between patterns")]
    AmbiguousMatch,
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::{assert_matches, make_map};

    use super::*;

    macro_rules! from_json {
        ($($json:tt)*) => {
            serde_json::from_value(serde_json::json!( $($json)* )).unwrap()
        };
    }

    #[test]
    fn test_rule_from_spec() {
        let spec: MappingRuleSpec = serde_json::from_value(serde_json::json!({
            "name": "test",
            "includes": ["**/*.rs"],
            "properties": {
                "type": "pic",
                "id": "102",
            }
        }))
        .unwrap();

        let rule = MappingRule::from_spec(&spec).unwrap();
        assert_eq!(rule.name(), Some("test"));
        assert_eq!(
            rule.apply_rule(Path::new("test.rs")).unwrap(),
            Some(make_map([("type", "pic"), ("id", "102")]))
        );
        assert_eq!(rule.apply_rule(Path::new("test.txt")).unwrap(), None);
        assert_eq!(
            rule.apply_rule(Path::new("some/other/directories/test.rs"))
                .unwrap(),
            Some(make_map([("type", "pic"), ("id", "102")]))
        );
    }

    #[test]
    fn test_rule_with_placeholders() {
        let spec: MappingRuleSpec = from_json!({
            "includes": ["**/{type}-{id}/*.rs"],
            "properties": {
                "type": "{type}",
                "id": "{id}",
                "rule": "test",
            }
        });

        let rule = MappingRule::from_spec(&spec).unwrap();
        assert_eq!(
            rule.apply_rule(Path::new("pic-102/test.rs")).unwrap(),
            Some(make_map([("type", "pic"), ("id", "102"), ("rule", "test")]))
        );
        assert_eq!(
            rule.apply_rule(Path::new("view-100/dir/test.rs")).unwrap(),
            None
        );
        assert_eq!(
            rule.apply_rule(Path::new("dir/view-100/test.rs")).unwrap(),
            Some(make_map([
                ("type", "view"),
                ("id", "100"),
                ("rule", "test")
            ]))
        );
    }

    #[test]
    fn test_with_excludes() {
        let spec: MappingRuleSpec = from_json!({
            "includes": ["**/*.rs"],
            "excludes": ["**/test.rs"],
            "properties": {}
        });

        let rule = MappingRule::from_spec(&spec).unwrap();
        assert_eq!(rule.apply_rule(Path::new("test.rs")).unwrap(), None);
        assert_eq!(
            rule.apply_rule(Path::new("some/other/directories/test.rs"))
                .unwrap(),
            None
        );
        assert_eq!(
            rule.apply_rule(Path::new("some/other/directories/other.rs"))
                .unwrap(),
            Some(BTreeMap::new())
        );
    }

    #[test]
    fn test_with_multiple_includes() {
        let spec: MappingRuleSpec = from_json!({
            "includes": [
                "**/{type}-{id}/*.rs",
                "**/*.{type}.{id}.rs"
            ],
            "properties": {
                "type": "{type}",
                "id": "{id}"
            }
        });

        let rule = MappingRule::from_spec(&spec).unwrap();
        assert_eq!(
            rule.apply_rule(Path::new("some/dir/test.view.100.rs"))
                .unwrap(),
            Some(make_map([("type", "view"), ("id", "100")]))
        );
        assert_eq!(
            rule.apply_rule(Path::new("dir/view-100/test.rs")).unwrap(),
            Some(make_map([("type", "view"), ("id", "100")]))
        );
        assert_matches!(
            rule.apply_rule(Path::new("dir/view-100/test.pic.200.rs")),
            Err(MappingError::AmbiguousMatch)
        );
    }

    #[test]
    fn test_with_invalid_specs() {
        macro_rules! fails_parse {
            ({$($json:tt)*}, $err:pat) => {
                let spec: MappingRuleSpec = from_json!({ $($json)* });
                let err = MappingRule::from_spec(&spec).unwrap_err();
                assert_matches!(err, $err);
            };
        }

        // No includes
        fails_parse!({
            "includes": [],
            "properties": {}
        }, SpecError::NoIncludePatterns);

        // Bad include
        fails_parse!({
            "includes": ["**{"],
            "properties": {}
        }, SpecError::InvalidPattern(_));

        // Includes with mismatched placeholders
        fails_parse!({
            "includes": [
                "**/{type}-{id}/*.rs",
                "**/*.txt",
            ],
            "properties": {}
        }, SpecError::MismatchedIncludePlaceholders);

        // Property placeholder that isn't taken from inputs
        fails_parse!({
            "includes": ["**/*.rs"],
            "properties": {
                "type": "{type}",
                "id": "{id}",
            }
        }, SpecError::UndefinedTemplatePlaceholder(_));
    }
}
