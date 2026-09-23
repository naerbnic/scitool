use std::{
    io,
    path::{Path, PathBuf},
};

use crate::relpath::{RelPath, RelPathBuf};

pub struct RelWalkDir {
    base: PathBuf,
    walker: walkdir::WalkDir,
}

impl RelWalkDir {
    pub fn new(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref().to_path_buf();
        Self {
            walker: walkdir::WalkDir::new(&base)
                .follow_links(false)
                .follow_root_links(false),
            base,
        }
    }
}

impl IntoIterator for RelWalkDir {
    type Item = io::Result<RelPathBuf>;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            base: self.base,
            iter: self.walker.into_iter(),
        }
    }
}

pub struct IntoIter {
    base: PathBuf,
    iter: walkdir::IntoIter,
}

impl Iterator for IntoIter {
    type Item = io::Result<RelPathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        (|| {
            Ok(Some(loop {
                let Some(entry) = self.iter.next().transpose()? else {
                    return Ok(None);
                };

                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry
                    .path()
                    .strip_prefix(&self.base)
                    .map_err(|e| io::Error::other(format!("Invalid relative path: {e}")))?;
                break RelPath::try_from_std_path(&path)
                    .ok_or_else(|| {
                        io::Error::other(format!("Invalid relative path: {}", path.display()))
                    })?
                    .to_buf();
            }))
        })()
        .transpose()
    }
}
