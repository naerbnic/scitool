//! Generalized search paths, to look up paths with different properties.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::Metadata,
    io,
};

use crate::path::{
    abspath::{AbsPath, AbsPathBuf},
    is_executable_with_metadata,
    relpath::{RelPath, RelPathBuf},
    walkdir::RelWalkDir,
};

fn get_opt_metadata(path: impl AsRef<AbsPath>) -> io::Result<Option<Metadata>> {
    match std::fs::metadata(path.as_ref().as_std()) {
        Ok(m) => Ok(Some(m)),
        Err(e) if let io::ErrorKind::NotFound = e.kind() => Ok(None),
        Err(e) => Err(e),
    }
}

pub struct SearchPath {
    paths: Vec<AbsPathBuf>,
}

impl SearchPath {
    pub fn new(paths: impl IntoIterator<Item = AbsPathBuf>) -> Self {
        Self {
            paths: paths.into_iter().collect(),
        }
    }

    /// Finds the first absolute path which contains the given relative path.
    pub fn find_first_with_path(
        &self,
        path: impl AsRef<RelPath>,
        options: &SearchOptions,
    ) -> io::Result<Option<AbsPathBuf>> {
        let path = path.as_ref();
        for base_path in &self.paths {
            let potential_path = base_path.join(path);
            if let Some(path_metadata) = get_opt_metadata(&potential_path)?
                && options.matches(&potential_path, path, &path_metadata)
            {
                return Ok(Some(potential_path));
            }
        }

        Ok(None)
    }

    /// Finds all absolute paths in the search paths which contain the given relative path.
    ///
    /// The results will be filtered by the given `SearchOptions`.
    pub fn find_all_with_path(
        &self,
        path: impl AsRef<RelPath>,
        options: &SearchOptions,
    ) -> io::Result<Vec<AbsPathBuf>> {
        let path = path.as_ref();
        let mut result = Vec::new();
        for base_path in &self.paths {
            let potential_path = base_path.join(path);
            if let Some(path_metadata) = get_opt_metadata(&potential_path)?
                && options.matches(&potential_path, path, &path_metadata)
            {
                result.push(potential_path);
            }
        }

        Ok(result)
    }

    /// Finds all files in all of the search paths that match the given options.
    /// Only the first file found with a given relative path is included in the results.
    ///
    /// Note that in any case, only the first file at the given rel path is tested by
    /// the options. Even if further files would match, they will not be included in the
    /// results.
    ///
    /// Returns a mapping from the relative path to the absolute path for all found files.
    pub fn find_all_as_overlay(
        &self,
        options: &SearchOptions,
    ) -> io::Result<BTreeMap<RelPathBuf, AbsPathBuf>> {
        let mut result = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for base_path in &self.paths {
            eprintln!("Base path: {base_path}");
            for entry in RelWalkDir::new(base_path) {
                let entry = entry?;
                eprintln!("Rel path: {}", entry.rel_path());
                let rel_path = entry.rel_path();
                let potential_path = AbsPathBuf::from_std(entry.full_path());
                if seen.contains(rel_path) {
                    continue;
                }
                seen.insert(rel_path.to_buf());
                let metadata = entry.metadata()?;
                if options.matches(&potential_path, rel_path, &metadata) {
                    result.insert(rel_path.to_buf(), potential_path);
                }
            }
        }
        Ok(result)
    }

    pub fn find_all(
        &self,
        options: &SearchOptions,
    ) -> io::Result<BTreeMap<RelPathBuf, Vec<AbsPathBuf>>> {
        let mut result = BTreeMap::new();
        for base_path in &self.paths {
            for entry in RelWalkDir::new(base_path) {
                let entry = entry?;
                let rel_path = entry.rel_path();
                let potential_path = AbsPathBuf::from_std(entry.full_path());
                let metadata = entry.metadata()?;
                if options.matches(&potential_path, rel_path, &metadata) {
                    result
                        .entry(rel_path.to_buf())
                        .or_insert_with(Vec::new)
                        .push(potential_path);
                }
            }
        }
        Ok(result)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SearchOptions {
    recurse: bool,
    extension: Option<String>,
    executable: Option<bool>,
    file_type: Option<FileType>,
    writable: Option<bool>,
}

impl SearchOptions {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn match_extension(mut self, extension: impl Into<String>) -> Self {
        self.extension = Some(extension.into());
        self
    }

    #[must_use]
    pub fn match_executable(mut self, should_be_executable: bool) -> Self {
        self.executable = Some(should_be_executable);
        self
    }

    #[must_use]
    pub fn match_file_type(mut self, file_type: FileType) -> Self {
        self.file_type = Some(file_type);
        self
    }

    #[must_use]
    pub fn match_writable(mut self, should_be_writable: bool) -> Self {
        self.writable = Some(should_be_writable);
        self
    }

    #[must_use]
    pub fn recurse(mut self, recurse: bool) -> Self {
        self.recurse = recurse;
        self
    }

    fn matches(&self, path: &AbsPath, rel_path: &RelPath, m: &Metadata) -> bool {
        if rel_path.components().count() > 1 && !self.recurse {
            return false;
        }

        if let Some(extension) = &self.extension
            && path.extension().unwrap_or("") == extension
        {
            return false;
        }

        if let Some(executable) = self.executable
            && is_executable_with_metadata(path, m) != executable
        {
            return false;
        }

        if let Some(file_type) = &self.file_type {
            match file_type {
                FileType::Directory if m.is_dir() => {}
                FileType::File if m.is_file() => {}
                _ => return false,
            }
        }

        if let Some(writable) = self.writable
            && m.permissions().readonly() == writable
        {
            return false;
        }

        true
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum FileType {
    File,
    Directory,
}
