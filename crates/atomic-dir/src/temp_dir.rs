use std::{io, path::Path, sync::Arc};

use cap_std::fs::Dir;
use rand::distr::SampleString as _;

use crate::{
    err_helpers::{io_err, io_err_map},
    paths::{SinglePath, SinglePathBuf},
    util::safe_path_parent,
};

#[derive(Debug)]
pub(super) struct TempDir {
    parent: Arc<Dir>,
    dir_root: Option<Dir>,
    dir_name: SinglePathBuf,
}

impl TempDir {
    pub(super) fn new_in(parent: Arc<Dir>, root_name: &SinglePath) -> io::Result<Self> {
        let dir_name = format!(
            ".{}.{}.tmpdir",
            root_name.display(),
            rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 10)
        );
        parent.create_dir(&dir_name)?;
        let dir = parent.open_dir(&dir_name)?;
        Ok(TempDir {
            parent,
            dir_root: Some(dir),
            dir_name: SinglePathBuf::new_checked(&dir_name).map_err(io_err_map!(InvalidInput))?,
        })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Unable to persist temporary directory: {cause}")]
pub(super) struct PersistError {
    pub(super) dir: TempDir,
    #[source]
    pub(super) cause: io::Error,
}

impl From<PersistError> for io::Error {
    fn from(err: PersistError) -> Self {
        err.cause
    }
}

impl TempDir {
    pub(super) fn dir_name(&self) -> &SinglePath {
        &self.dir_name
    }

    pub(super) fn defuse(mut self) -> DefusedTempDir {
        let dir = self.dir_root.take().expect("TempDir is valid");
        DefusedTempDir {
            temp_dir: self,
            dir,
        }
    }

    pub(super) fn persist_to(mut self, path: &Path) -> Result<(), PersistError> {
        macro_rules! try_persist {
            ($expr:expr) => {
                match $expr {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(PersistError {
                            dir: self,
                            cause: e,
                        });
                    }
                }
            };
        }
        let Some((target_parent, target_name)) = try_persist!(safe_path_parent(path)) else {
            return Err(PersistError {
                dir: self,
                cause: io_err!(InvalidInput, "Path has no parent: {}", path.display()),
            });
        };
        let target_parent_dir = try_persist!(Dir::open_ambient_dir(
            target_parent,
            cap_std::ambient_authority()
        ));
        try_persist!(
            self.parent
                .rename(&self.dir_name, &target_parent_dir, target_name)
        );
        // Disarm the destructor so we don't try to delete the directory.
        self.dir_root = None;
        Ok(())
    }
}

impl std::ops::Deref for TempDir {
    type Target = Dir;

    fn deref(&self) -> &Self::Target {
        self.dir_root.as_ref().expect("TempDir is valid")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // If we weren't invalidated in one of the persist methods, clean up
        // the temporary directory.
        if let Some(_dir) = self.dir_root.take() {
            drop(self.parent.remove_dir_all(&self.dir_name));
        }
    }
}

pub(crate) struct DefusedTempDir {
    temp_dir: TempDir,
    dir: Dir,
}

impl DefusedTempDir {
    pub(crate) fn relight(mut self) -> TempDir {
        self.temp_dir.dir_root = Some(self.dir);
        self.temp_dir
    }
}
