use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use unicode_normalization::UnicodeNormalization;

use crate::project::paths::regex::UnambiguousRegex;

#[derive(Debug, thiserror::Error)]
pub enum MatchError {
    #[error("String matched ambiguously with the capture groups.")]
    AmbiguousMatch,
}

#[derive(Debug, Clone)]
pub struct FileSetEntry {
    path: PathBuf,
    properties: BTreeMap<String, String>,
}

impl FileSetEntry {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn properties(&self) -> &BTreeMap<String, String> {
        &self.properties
    }
}

pub struct PathMatcher {
    matcher: UnambiguousRegex,
    captures: Vec<String>,
}

impl PathMatcher {
    pub(super) fn new(matcher: UnambiguousRegex, captures: Vec<String>) -> Self {
        Self { matcher, captures }
    }

    pub fn match_path(&self, path: impl Into<PathBuf>) -> Result<Option<FileSetEntry>, MatchError> {
        let path = path.into();
        // Create a syntactically canonical path, using "/" as the separator.
        let path_str = path
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
        for (capture_name, capture_span) in self.captures.iter().zip(captures.iter().skip(1)) {
            let Some(capture_span) = capture_span else {
                panic!("All capture values should be present");
            };
            let capture_value = &path_str[capture_span.range()];
            properties.insert(capture_name.clone(), capture_value.to_string());
        }

        Ok(Some(FileSetEntry { path, properties }))
    }
}
