use std::{
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub(crate) fn is_relative_path(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    for component in path.components() {
        match component {
            std::path::Component::Normal(_) => {}
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::CurDir
            | std::path::Component::ParentDir => return false,
        }
    }
    true
}

struct EntityInfo<'a> {
    full_path: &'a Path,
    rel_path: &'a Path,
    permissions: std::fs::Permissions,
    accessed: Option<SystemTime>,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
}

pub(crate) struct FileInfo<'a> {
    len: u64,
    entity_info: EntityInfo<'a>,
}

impl FileInfo<'_> {
    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn file_size(&self) -> u64 {
        self.len
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn full_path(&self) -> &Path {
        self.entity_info.full_path
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn path(&self) -> &Path {
        self.entity_info.rel_path
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn is_readonly(&self) -> bool {
        self.entity_info.permissions.readonly()
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn created(&self) -> Option<SystemTime> {
        self.entity_info.created
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn modified(&self) -> Option<SystemTime> {
        self.entity_info.modified
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn accessed(&self) -> Option<SystemTime> {
        self.entity_info.accessed
    }
}

pub(crate) trait ValidResult<T, E> {
    fn into_result(self) -> Result<T, E>;
}

impl<T, E> ValidResult<T, E> for T {
    fn into_result(self) -> Result<T, E> {
        Ok(self)
    }
}

impl<T, E> ValidResult<T, E> for Result<T, E> {
    fn into_result(self) -> Result<T, E> {
        self
    }
}

pub(crate) struct DirInfo<'a> {
    entity_info: EntityInfo<'a>,
}

impl DirInfo<'_> {
    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn full_path(&self) -> &Path {
        self.entity_info.full_path
    }

    pub(crate) fn path(&self) -> &Path {
        self.entity_info.rel_path
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn created(&self) -> Option<SystemTime> {
        self.entity_info.created
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn modified(&self) -> Option<SystemTime> {
        self.entity_info.modified
    }
}

#[expect(clippy::type_complexity, reason = "Unable to create local type alias")]
pub(crate) struct FileLister<'a> {
    root: PathBuf,
    file_filter: Box<dyn for<'b> Fn(&FileInfo<'b>) -> io::Result<bool> + 'a>,
    dir_filter: Box<dyn for<'b> Fn(&DirInfo<'b>) -> io::Result<bool> + 'a>,
    should_recurse: Box<dyn for<'b> Fn(&DirInfo<'b>) -> io::Result<bool> + 'a>,
}

impl<'a> FileLister<'a> {
    pub(crate) fn new(root: impl AsRef<Path> + 'a) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            file_filter: Box::new(|_| Ok(true)),
            dir_filter: Box::new(|_| Ok(false)),
            should_recurse: Box::new(|_| Ok(true)),
        }
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn set_file_filter<R>(
        &mut self,
        file_filter: impl for<'b> Fn(&FileInfo<'b>) -> R + 'a,
    ) -> &mut Self
    where
        R: ValidResult<bool, io::Error>,
    {
        self.file_filter = Box::new(move |info| file_filter(info).into_result());
        self
    }

    pub(crate) fn set_dir_filter<R>(
        &mut self,
        dir_filter: impl for<'b> Fn(&DirInfo<'b>) -> R + 'a,
    ) -> &mut Self
    where
        R: ValidResult<bool, io::Error>,
    {
        self.dir_filter = Box::new(move |info| dir_filter(info).into_result());
        self
    }

    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn set_should_recurse<R>(
        &mut self,
        should_recurse: impl for<'b> Fn(&DirInfo<'b>) -> R + 'a,
    ) -> &mut Self
    where
        R: ValidResult<bool, io::Error>,
    {
        self.should_recurse = Box::new(move |info| should_recurse(info).into_result());
        self
    }

    pub(crate) fn list_all(&self) -> io::Result<Vec<PathBuf>> {
        let mut file_iter = walkdir::WalkDir::new(&self.root).into_iter();

        let mut paths = Vec::new();

        while let Some(entry) = file_iter.next() {
            let entry = entry?;
            // Ignore the root path itself.
            if entry.path() == self.root {
                continue;
            }
            let rel_path = entry
                .path()
                .strip_prefix(&self.root)
                .expect("Docs require prefix");
            assert!(is_relative_path(rel_path));
            let metadata = entry.metadata()?;
            let entity_info = EntityInfo {
                full_path: entry.path(),
                rel_path,
                permissions: metadata.permissions(),
                accessed: metadata.accessed().ok(),
                modified: metadata.modified().ok(),
                created: metadata.created().ok(),
            };
            if entry.file_type().is_dir() {
                let dir_info = DirInfo { entity_info };
                if (self.dir_filter)(&dir_info)? {
                    paths.push(rel_path.to_path_buf());
                }
                if !(self.should_recurse)(&dir_info)? {
                    file_iter.skip_current_dir();
                }
            } else if entry.file_type().is_file() {
                let file_info = FileInfo {
                    len: metadata.len(),
                    entity_info,
                };
                if (self.file_filter)(&file_info)? {
                    paths.push(rel_path.to_path_buf());
                }
            }
        }
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use crate::helpers::{iter::eq_unordered, test::build_files};

    use super::*;

    #[test]
    fn test_default_file_lister() -> io::Result<()> {
        let root = build_files!(
            "test.txt" => "Hello, World\n",
            "dir" => {
                "f1.txt", "f2.txt"
            }
        );

        let files = FileLister::new(root.path()).list_all()?;
        assert!(eq_unordered(
            files.iter().map(|p| p.to_str().unwrap()),
            ["test.txt", "dir/f1.txt", "dir/f2.txt"]
        ));

        Ok(())
    }

    #[test]
    fn test_file_lister_with_dir() -> io::Result<()> {
        let root = build_files!(
            "test.txt" => "Hello, World\n",
            "dir" => {
                "f1.txt", "f2.txt"
            }
        );

        let files = FileLister::new(root.path())
            .set_dir_filter(|_| Ok(true))
            .list_all()?;
        assert!(eq_unordered(
            files.iter().map(|p| p.to_str().unwrap()),
            ["dir", "test.txt", "dir/f1.txt", "dir/f2.txt"]
        ));

        Ok(())
    }
}
