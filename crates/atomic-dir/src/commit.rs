use std::{io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    CreateMode, DirLock, err_helpers::io_bail, paths::SinglePathBuf, util::write_file_atomic_at,
};

pub(super) const CURR_COMMIT_VERSION: u32 = 1;

const COMMIT_FILE_SUFFIX: &str = ".commit";

#[derive(Debug, Serialize, Deserialize)]
struct CommitContents {
    /// The version of the commit schema, in case it changes.
    version: u32,

    /// The location of the temp file that was being moved during the commit.
    ///
    /// Must be a directory only, i.e. a relative path with a single normal component.
    temp_dir: SinglePathBuf,

    /// The location the old directory will be moved to.
    ///
    /// Must be a directory only, i.e. a relative path with a single normal component.
    old_dir: SinglePathBuf,
}

impl CommitContents {
    fn validate(&self) -> io::Result<()> {
        if self.version != 1 {
            io_bail!(
                InvalidData,
                "Unsupported commit schema version: {}",
                self.version
            );
        }

        Ok(())
    }
}

fn get_commit_file_path(dir_lock: &DirLock) -> PathBuf {
    dir_lock.adjacent_ext_path(COMMIT_FILE_SUFFIX)
}

pub(super) struct CommitFileData {
    contents: CommitContents,
}

impl CommitFileData {
    pub(super) fn read_at(lock: &DirLock) -> io::Result<Option<Self>> {
        // The lock should protect against concurrent access to the commit file.
        match lock.parent_dir().read(get_commit_file_path(lock)) {
            Ok(data) => {
                let contents: CommitContents = serde_json::from_slice(&data)?;
                contents.validate()?;
                Ok(Some(Self { contents }))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(super) fn temp_dir(&self) -> &SinglePathBuf {
        &self.contents.temp_dir
    }
    pub(super) fn old_dir(&self) -> &SinglePathBuf {
        &self.contents.old_dir
    }

    pub(super) fn with_old_dir(&self, new_old_dir: SinglePathBuf) -> Self {
        Self {
            contents: CommitContents {
                version: self.contents.version,
                temp_dir: self.contents.temp_dir.clone(),
                old_dir: new_old_dir,
            },
        }
    }

    pub(super) fn new(temp_dir: SinglePathBuf, old_dir: SinglePathBuf) -> Self {
        Self {
            contents: CommitContents {
                version: CURR_COMMIT_VERSION,
                temp_dir,
                old_dir,
            },
        }
    }

    pub(super) fn commit_file(&self, path: &DirLock) -> io::Result<()> {
        let commit_file_path = get_commit_file_path(path);
        let data = serde_json::to_vec(&self.contents)?;
        // After the commit file is written, the directory should be durable.
        write_file_atomic_at(
            path.parent_dir(),
            &commit_file_path,
            &data,
            CreateMode::CreateNew,
        )?;
        Ok(())
    }
}
