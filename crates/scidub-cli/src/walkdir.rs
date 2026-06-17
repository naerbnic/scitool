use std::path::{Path, PathBuf};

use sciproj::path::relpath::{RelPath, RelPathBuf};

pub(crate) struct RelWalkDir {
    base: PathBuf,
    iter: walkdir::IntoIter,
}

impl RelWalkDir {
    pub(crate) fn new(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref().to_path_buf();
        Self {
            iter: walkdir::WalkDir::new(&base)
                .follow_links(false)
                .follow_root_links(false)
                .into_iter(),
            base,
        }
    }
}

impl Iterator for RelWalkDir {
    type Item = anyhow::Result<RelPathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        (|| {
            Ok(Some(loop {
                let Some(entry) = self.iter.next().transpose()? else {
                    return Ok(None);
                };

                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry.path().strip_prefix(&self.base)?;
                break RelPath::try_from_std_path(&path)
                    .ok_or_else(|| anyhow::anyhow!("Invalid relative path: {}", path.display()))?
                    .to_buf();
            }))
        })()
        .transpose()
    }
}
