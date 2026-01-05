use std::collections::BTreeMap;

use crate::project::file_mapping::rule::{self, MappingRule, MappingRuleSpec};

#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    #[error("Invalid rule: {0}")]
    InvalidRule(#[from] rule::SpecError),

    #[error("Undefined override: {0}")]
    UndefinedOverride(String),
}

pub type RuleSetSpec = Vec<MappingRuleSpec>;

pub struct RuleSet(Vec<MappingRule>);

impl RuleSet {
    pub fn from_spec<'a>(
        spec: impl IntoIterator<Item = &'a MappingRuleSpec>,
    ) -> Result<Self, SpecError> {
        let mut rules = Vec::new();
        let mut rule_name_indexes = BTreeMap::new();
        for (i, rule_spec) in spec.into_iter().enumerate() {
            let rule = MappingRule::from_spec(rule_spec)?;
            if let Some(name) = rule.name() {
                rule_name_indexes.insert(name.to_string(), i);
            }
            rules.push(rule);
        }

        // Check for existence of overrides.
        for rule in &rules {
            for override_name in rule.overrides() {
                if !rule_name_indexes.contains_key(override_name) {
                    return Err(SpecError::UndefinedOverride(override_name.to_string()));
                }
            }
        }

        // Check for cycles between overrides.
        // TODO

        // Sort rules topologically.
        // TODO

        Ok(Self(rules))
    }
}
