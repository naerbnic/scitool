use std::{collections::BTreeMap, ops::Range, path::Path};

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
    matchers: Vec<UnambiguousRegex>,
    captures: Vec<String>,
}

impl PathMatcher {
    pub(super) fn new(matchers: Vec<UnambiguousRegex>, captures: Vec<String>) -> Self {
        Self { matchers, captures }
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

        let capture_spans = self
            .matchers
            .iter()
            .map(|matcher| {
                matcher
                    .match_unambiguous(&path_str)
                    .map_err(|_| MatchError::AmbiguousMatch)
            })
            .filter_map(Result::transpose)
            .reduce(|a, b| {
                // Check that all valid matches match the exact same spans of the
                // string.
                let a = a?;
                let b = b?;
                if a == b {
                    Ok(a)
                } else {
                    Err(MatchError::AmbiguousMatch)
                }
            })
            .transpose()?;

        let Some(capture_spans) = capture_spans else {
            return Ok(None);
        };

        let captures = capture_spans
            .extract(&path_str)
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();

        Ok(Some(MatchResult {
            normalized_path: path_str,
            captures,
        }))
    }
}
