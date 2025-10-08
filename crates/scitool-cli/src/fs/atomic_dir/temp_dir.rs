use std::{io, sync::Arc};

use cap_std::fs::Dir;
use rand::distr::SampleString as _;

use crate::fs::{
    err_helpers::io_err_map,
    paths::{SinglePath, SinglePathBuf},
};

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

impl TempDir {
    pub(super) fn dir_name(&self) -> &SinglePath {
        &self.dir_name
    }

    pub(super) fn into_dir(mut self) -> Dir {
        self.dir_root.take().expect("TempDir is valid")
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
