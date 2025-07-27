//! Tools for working with system paths, such as looking up a binary in the PATH
//! environment variable.

use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

#[cfg(unix)]
mod plat {
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    pub fn is_executable(path: &Path) -> bool {
        path.is_file()
            && path
                .metadata()
                .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }

    pub fn binary_name(path: &Path) -> &OsStr {
        path.file_name()
            .unwrap_or_else(|| panic!("Path {} does not have a file name", path.display(),))
    }
}

#[cfg(windows)]
mod plat {
    use std::{ffi::OsStr, path::Path};

    pub fn is_executable(path: &Path) -> bool {
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

    pub fn binary_name(path: &Path) -> &OsStr {
        let Some(stem) = path.file_stem() else {
            panic!("Path {:?} does not have a file name", path);
        };
        stem
    }
}

pub struct LookupPath {
    path_entries: HashMap<OsString, PathBuf>,
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
        let mut path_entries = HashMap::new();
        for base_path in paths {
            if base_path.is_dir() {
                let Ok(dir_entries) = base_path.read_dir() else {
                    continue;
                };

                for entry in dir_entries {
                    let Ok(entry) = entry else {
                        continue;
                    };
                    let path = entry.path();
                    if plat::is_executable(&path) {
                        let bin_name = plat::binary_name(&path).to_os_string();
                        path_entries.entry(bin_name).or_insert(path);
                    }
                }
            }
        }
        LookupPath { path_entries }
    }

    pub fn list_binaries(&self) -> impl Iterator<Item = (&OsString, &Path)> + use<'_> {
        self.path_entries
            .iter()
            .map(|(name, path)| (name, path.as_path()))
    }

    pub fn find_binary(&self, name: impl AsRef<OsStr>) -> Option<&Path> {
        self.path_entries.get(name.as_ref()).map(PathBuf::as_path)
    }

    pub fn has_binary(&self, name: &impl AsRef<OsStr>) -> bool {
        self.path_entries.contains_key(name.as_ref())
    }
}
