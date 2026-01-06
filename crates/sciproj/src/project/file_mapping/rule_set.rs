use std::{collections::BTreeMap, path::Path};

use crate::project::file_mapping::rule::{self, MappingRule, MappingRuleSpec};

#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    #[error("Invalid rule: {0}")]
    InvalidRule(#[from] rule::SpecError),

    #[error("Undefined override: {0}")]
    UndefinedOverride(String),

    #[error("Cycle detected")]
    CycleDetected,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("Rule failed: {0}")]
    RuleFailed(#[from] rule::MappingError),

    #[error("Property collision")]
    PropertyCollision,
}

pub type RuleSetSpec = Vec<MappingRuleSpec>;

#[derive(Debug)]
pub struct RuleSet(Vec<MappingRule>);

impl RuleSet {
    #[expect(single_use_lifetimes, reason = "false positive")]
    pub fn from_spec<'a>(
        spec: impl IntoIterator<Item = &'a MappingRuleSpec>,
    ) -> Result<Self, SpecError> {
        let mut rules = Vec::new();
        for rule_spec in spec {
            rules.push(MappingRule::from_spec(rule_spec)?);
        }

        topo_sort::<MappingRule, _>(
            &mut rules,
            |rule: &MappingRule| rule.name(),
            |rule: &MappingRule| rule.overrides().collect(),
        )
        .map_err(|e| match e {
            SortError::UndefinedEdge(key) => SpecError::UndefinedOverride(key),
            SortError::CycleDetected => SpecError::CycleDetected,
        })?;

        Ok(Self(rules))
    }

    /// Apply this rule set to the given path, aggregating its properties in
    /// `prop_map`.
    ///
    /// The properties in prop_map on input are assumed to override any
    /// properties set by this rule set.
    pub fn apply(
        &self,
        path: impl AsRef<Path>,
        prop_map: &mut BTreeMap<String, String>,
    ) -> Result<(), ApplyError> {
        let path = path.as_ref();
        let mut local_props = BTreeMap::new();
        let mut overridden_rules = Vec::new();

        for rule in &self.0 {
            // Skip this rule if its properties are already set in the outer prop_map.
            //
            // This enforces the rule that nested rules override outer rules.
            if rule.properties().any(|prop| prop_map.contains_key(prop)) {
                break;
            }

            // Skip this rule if it has been overridden by a previous rule.
            if let Some(name) = rule.name()
                && overridden_rules.contains(&name)
            {
                break;
            }

            if let Some(rule_props) = rule.apply_rule(path)? {
                // Check that we aren't overwriting a property set by a previous rule.
                //
                // Any priority overrides should already have been applied.
                if rule.properties().any(|prop| local_props.contains_key(prop)) {
                    return Err(ApplyError::PropertyCollision);
                }

                local_props.extend(rule_props);
                overridden_rules.extend(rule.overrides());
            }
        }

        prop_map.extend(local_props);

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SortError<K> {
    #[error("Undefined edge")]
    UndefinedEdge(K),

    #[error("Cycle detected")]
    CycleDetected,
}

fn topo_sort<T, K>(
    items: &mut [T],
    key_fn: impl for<'b> Fn(&'b T) -> Option<&'b K>,
    edge_fn: impl for<'b> Fn(&'b T) -> Vec<&'b K>,
) -> Result<(), SortError<K::Owned>>
where
    K: Ord + ToOwned + ?Sized,
{
    let mut sorted_indices;
    {
        let mut key_indices = BTreeMap::new();
        for (i, item) in items.iter().enumerate() {
            if let Some(key) = key_fn(item) {
                key_indices.insert(key, i);
            }
        }

        let mut adjacency = vec![vec![]; items.len()];
        let mut in_degree = vec![0; items.len()];

        for (i, item) in items.iter().enumerate() {
            for key in edge_fn(item) {
                let Some(&target) = key_indices.get(key) else {
                    return Err(SortError::UndefinedEdge(key.to_owned()));
                };

                adjacency[i].push(target);
                in_degree[target] += 1;
            }
        }

        let mut queue = Vec::new();
        for (i, &degree) in in_degree.iter().enumerate() {
            if degree == 0 {
                queue.push(i);
            }
        }

        sorted_indices = Vec::with_capacity(items.len());
        let mut head = 0;
        while head < queue.len() {
            let u = queue[head];
            head += 1;
            sorted_indices.push(u);

            for &v in &adjacency[u] {
                in_degree[v] -= 1;
                if in_degree[v] == 0 {
                    queue.push(v);
                }
            }
        }
    }

    if sorted_indices.len() != items.len() {
        return Err(SortError::CycleDetected);
    }

    // Reorder items based on sorted_indices.
    let mut pos = vec![0; items.len()];
    for (new_idx, &old_idx) in sorted_indices.iter().enumerate() {
        pos[old_idx] = new_idx;
    }

    for i in 0..items.len() {
        while pos[i] != i {
            let dest = pos[i];
            items.swap(i, dest);
            pos.swap(i, dest);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{assert_matches, from_json, make_map};

    #[test]
    fn test_topo_sort() {
        let spec: RuleSetSpec = from_json!([
            {
                "name": "base",
                "includes": ["**/*.txt"],
                "properties": {"type": "txt"}
            },
            {
                "name": "override_1",
                "includes": ["**/*.txt"],
                "properties": {"type": "ovr1"},
                "overrides": ["base"]
            },
            {
                "name": "override_2",
                "includes": ["**/*.txt"],
                "properties": {"type": "ovr2"},
                "overrides": ["override_1"]
            }
        ]);

        let rule_set = RuleSet::from_spec(&spec).unwrap();
        let rules = &rule_set.0;
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].name(), Some("override_2"));
        assert_eq!(rules[1].name(), Some("override_1"));
        assert_eq!(rules[2].name(), Some("base"));
    }

    #[test]
    fn test_cycle_detection() {
        let spec: RuleSetSpec = from_json!([
            {
                "name": "a",
                "includes": ["**/*.txt"],
                "properties": {"type": "a"},
                "overrides": ["b"]
            },
            {
                "name": "b",
                "includes": ["**/*.txt"],
                "properties": {"type": "b"},
                "overrides": ["a"]
            }
        ]);

        let err = RuleSet::from_spec(&spec).unwrap_err();
        assert_matches!(err, SpecError::CycleDetected);
    }

    #[test]
    fn test_cycle_detection_self() {
        let spec: RuleSetSpec = from_json!([
            {
                "name": "a",
                "includes": ["**/*.txt"],
                "properties": {"type": "a"},
                "overrides": ["a"]
            }
        ]);

        let err = RuleSet::from_spec(&spec).unwrap_err();
        assert_matches!(err, SpecError::CycleDetected);
    }

    #[test]
    fn test_single_rule_apply() {
        let spec: RuleSetSpec = from_json!([
            {
                "includes": ["**/*.txt"],
                "properties": {"type": "txt"}
            }
        ]);

        let rule_set = RuleSet::from_spec(&spec).unwrap();
        let mut prop_map = BTreeMap::new();
        rule_set.apply("test.txt", &mut prop_map).unwrap();
        assert_eq!(prop_map, make_map([("type", "txt")]));
    }

    #[test]
    fn test_disjoint_rule_apply() {
        let spec: RuleSetSpec = from_json!([
            {
                "includes": ["**/*.txt"],
                "properties": {"type": "txt"}
            },
            {
                "includes": ["**/*.rs"],
                "properties": {"type": "rs"}
            }
        ]);

        let rule_set = RuleSet::from_spec(&spec).unwrap();
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("test.txt", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("type", "txt")]));
        }
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("test.rs", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("type", "rs")]));
        }
    }

    #[test]
    fn test_merged_rule_apply() {
        // If the property sets are disjoint, the properties are merged.
        let spec: RuleSetSpec = from_json!([
            {
                "includes": ["**/*.txt"],
                "properties": {"type": "txt"}
            },
            {
                "includes": ["**/src/**"],
                "properties": {"in_src": "yes"}
            }
        ]);

        let rule_set = RuleSet::from_spec(&spec).unwrap();
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("dir/test.txt", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("type", "txt")]));
        }
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("dir/src/test.rs", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("in_src", "yes")]));
        }
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("dir/src/test.txt", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("in_src", "yes"), ("type", "txt")]));
        }
    }

    #[test]
    fn test_conflicting_rule_apply() {
        // If the property sets are disjoint, the properties are merged.
        let spec: RuleSetSpec = from_json!([
            {
                "includes": ["**/*.txt"],
                "properties": {"type": "txt"}
            },
            {
                "includes": ["**/src/**"],
                "properties": {"type": "src"}
            }
        ]);

        let rule_set = RuleSet::from_spec(&spec).unwrap();
        // If only one rule triggers, no error is returned.
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("dir/test.txt", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("type", "txt")]));
        }
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("dir/src/test.rs", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("type", "src")]));
        }

        // If properties would write to the same property, an error is returned.
        {
            let mut prop_map = BTreeMap::new();
            assert_matches!(
                rule_set.apply("dir/src/test.txt", &mut prop_map),
                Err(ApplyError::PropertyCollision)
            );
        }
    }

    #[test]
    fn test_overridden_rule_apply() {
        // If the property sets are disjoint, the properties are merged.
        let spec: RuleSetSpec = from_json!([
            {
                "name": "base",
                "includes": ["**/*.txt"],
                "properties": {"type": "txt"}
            },
            {
                "includes": ["**/src/**"],
                "properties": {"type": "src"},
                "overrides": ["base"]
            }
        ]);

        let rule_set = RuleSet::from_spec(&spec).unwrap();
        // If only one rule triggers, it operates as expected.
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("dir/test.txt", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("type", "txt")]));
        }
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("dir/src/test.rs", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("type", "src")]));
        }

        // The overridden rule does not apply if both rules trigger.
        {
            let mut prop_map = BTreeMap::new();
            rule_set.apply("dir/src/test.txt", &mut prop_map).unwrap();
            assert_eq!(prop_map, make_map([("type", "src")]));
        }
    }
}
