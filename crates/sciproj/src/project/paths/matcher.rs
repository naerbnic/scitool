use std::{collections::BTreeMap, path::Path};

use unicode_normalization::UnicodeNormalization;

use crate::project::paths::regex::UnambiguousRegex;

#[derive(Debug, thiserror::Error)]
pub enum MatchError {
    #[error("String matched ambiguously with the capture groups.")]
    AmbiguousMatch,
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    normalized_path: String,
    captures: BTreeMap<String, String>,
}

impl MatchResult {
    pub fn normalized_path(&self) -> &str {
        &self.normalized_path
    }

    pub fn properties(&self) -> &BTreeMap<String, String> {
        &self.captures
    }
}

#[derive(Debug)]
pub(crate) struct PathMatcher {
    matcher: UnambiguousRegex,
    captures: Vec<String>,
}

impl PathMatcher {
    pub(super) fn new(matcher: UnambiguousRegex, captures: Vec<String>) -> Self {
        Self { matcher, captures }
    }

    pub fn placeholders(&self) -> &[String] {
        &self.captures
    }

    pub fn match_path(&self, path: impl AsRef<Path>) -> Result<Option<MatchResult>, MatchError> {
        // Create a syntactically canonical path, using "/" as the separator.
        let path_str = path
            .as_ref()
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
            .nfc()
            .collect::<String>();

        let captures = self
            .matcher
            .match_unambiguous(&path_str)
            .map_err(|_| MatchError::AmbiguousMatch)?;

        let Some(captures) = captures else {
            return Ok(None);
        };

        let mut properties = BTreeMap::new();
        // The first match is the entire string, so we skip it.
        for capture_name in &self.captures {
            let capture_span = captures.get_group_by_name(capture_name);
            let Some(capture_span) = capture_span else {
                panic!("All capture values should be present");
            };
            let capture_value = &path_str[capture_span.range()];
            properties.insert(capture_name.clone(), capture_value.to_string());
        }

        Ok(Some(MatchResult {
            normalized_path: path_str,
            captures: properties,
        }))
    }
}
