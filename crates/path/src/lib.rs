//! Tools for working with system paths, such as looking up a binary in the PATH
//! environment variable.

pub mod abspath;
pub mod relpath;
pub mod search;
pub mod segment;
pub mod walkdir;

use std::{
    collections::HashMap,
    fs::Metadata,
    io,
    path::{Path, PathBuf},
};

use crate::{
    abspath::AbsPathBuf,
    search::{SearchOptions, SearchPath},
};

#[cfg(unix)]
mod plat {
    use std::ffi::OsStr;
    use std::fs::Metadata;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    pub(crate) fn is_executable(path: &Path, m: &Metadata) -> bool {
        path.is_file() && m.permissions().mode() & 0o111 != 0
    }

    pub(crate) fn binary_name(path: &Path) -> &OsStr {
        path.file_name()
            .unwrap_or_else(|| panic!("Path {} does not have a file name", path.display()))
    }
}

#[cfg(windows)]
mod plat {
    use std::fs::Metadata;
    use std::{ffi::OsStr, path::Path};

    pub(super) fn is_executable(path: &Path, _m: &Metadata) -> bool {
        if !path.is_file() {
            return false;
        }

        let Some(ext) = path.extension() else {
            return false;
        };

        let Some(ext) = ext.to_str() else {
            return false;
        };

        ext.eq_ignore_ascii_case("exe")
    }

    pub(super) fn binary_name(path: &Path) -> &OsStr {
        let Some(stem) = path.file_stem() else {
            panic!("Path {:?} does not have a file name", path);
        };
        stem
    }
}

fn result_not_found_to_opt<T>(v: io::Result<T>) -> io::Result<Option<T>> {
    match v {
        Ok(v) => Ok(Some(v)),
        Err(e) if matches!(e.kind(), io::ErrorKind::NotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

#[must_use]
pub fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    let Ok(m) = path.metadata() else {
        return false;
    };
    plat::is_executable(path, &m)
}

#[must_use]
pub fn is_executable_with_metadata<P: AsRef<Path>>(path: P, m: &Metadata) -> bool {
    plat::is_executable(path.as_ref(), m)
}

#[derive(Clone, Debug)]
pub struct LookupPath {
    path_entries: HashMap<String, PathBuf>,
}

impl LookupPath {
    #[must_use]
    pub fn empty() -> Self {
        LookupPath {
            path_entries: HashMap::new(),
        }
    }

    #[must_use]
    pub fn from_env() -> Self {
        let Ok(path) = std::env::var("PATH") else {
            return LookupPath::empty();
        };
        Self::from_paths(std::env::split_paths(&path))
    }

    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let paths = paths.into_iter().collect::<Vec<_>>();
        let search_path = SearchPath::new(
            paths
                .clone()
                .into_iter()
                .filter_map(|p| result_not_found_to_opt(AbsPathBuf::new_canonical(&p)).unwrap()),
        );
        let bin_paths = search_path
            .find_all_as_overlay(
                &SearchOptions::empty()
                    .recurse(false)
                    .match_executable(true)
                    .ignore_access_errors(true),
            )
            .unwrap();

        let path_entries = bin_paths
            .into_iter()
            .map(|(k, v)| {
                (
                    plat::binary_name(k.as_std()).to_str().unwrap().to_string(),
                    v.into_std(),
                )
            })
            .collect();
        LookupPath { path_entries }
    }

    pub fn list_binaries(&self) -> impl Iterator<Item = (&String, &Path)> + use<'_> {
        self.path_entries
            .iter()
            .map(|(name, path)| (name, path.as_path()))
    }

    pub fn find_binary(&self, name: impl AsRef<str>) -> Option<&Path> {
        self.path_entries.get(name.as_ref()).map(PathBuf::as_path)
    }

    pub fn has_binary(&self, name: &impl AsRef<str>) -> bool {
        self.path_entries.contains_key(name.as_ref())
    }
}
