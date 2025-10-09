pub mod ephemeral;
mod err_helpers;
mod shared_lock_set;

use std::{
    fs::File,
    io::{self, Read, Seek as _, SeekFrom, Write as _},
};

use cap_std::fs::{Dir, OpenOptions};
use serde::{Deserialize, Serialize};

use crate::{
    err_helpers::{io_bail, io_err},
    shared_lock_set::Lock,
};

pub use shared_lock_set::LockType;

/// The contents of the lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockContents {
    /// The version of the lock file format.
    version: u32,

    /// The revision of the directory that is being modified. This can be used to
    /// detect if the directory has been modified since it was last read on lock
    /// update.
    revision: u32,

    /// If true, the directory has changed its location since the last time it was
    /// read. This can be used to detect if the directory has been comitted
    /// between the time that the attempt to lock the file started, and the
    /// time that the lock is acquired.
    poisoned: bool,
}

#[derive(Debug)]
struct LockState {
    revision: u32,
    updated: bool,
    poisoned: bool,
}

impl LockState {
    #[must_use]
    pub(self) fn new_fresh() -> Self {
        Self {
            revision: 0,
            updated: false,
            poisoned: false,
        }
    }

    pub(self) fn new_from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let contents: LockContents = serde_json::from_slice(bytes)?;
        if contents.version != 1 {
            io_bail!(Other, "Unsupported lock file version: {}", contents.version);
        }
        if contents.poisoned {
            io_bail!(Other, "Lock file is poisoned");
        }
        Ok(Self {
            revision: contents.revision,
            updated: false,
            poisoned: false,
        })
    }

    pub(self) fn to_bytes(&self) -> io::Result<Vec<u8>> {
        let bytes = serde_json::to_vec(&LockContents {
            version: 1,
            revision: self.revision,
            poisoned: self.poisoned,
        })?;
        Ok(bytes)
    }

    #[must_use]
    pub(self) fn revision(&self) -> u32 {
        self.revision
    }

    pub(self) fn mark_as_written(&mut self) -> bool {
        // Ensure we only increment the revision if it has not been modified.
        if self.updated {
            return false;
        }
        self.updated = true;

        // We allow wrapping add, to avoid dealing with overflow errors. There
        // is no practical risk of this happening in real world usage, either of
        // the revision wrapping around, or of the revision wrapping around
        // between initial read lock, and later write lock.
        self.revision = self.revision.wrapping_add(1);

        // Indicate that the revision was updated.
        true
    }

    pub(self) fn mark_as_poisoned(&mut self) -> bool {
        if self.poisoned {
            return false;
        }
        self.poisoned = true;
        true
    }
}

#[derive(Debug)]
pub struct LockFile {
    lock: Option<Lock>,
    lock_state: LockState,
}

impl LockFile {
    fn inner_file(&mut self) -> &mut File {
        self.lock.as_mut().unwrap()
    }
    pub fn create_in(root_dir: &Dir, lock_name: &str) -> io::Result<Self> {
        let file = root_dir.open_with(
            lock_name,
            OpenOptions::new().create_new(true).write(true).read(true),
        )?;
        let file = file.into_std();
        let mut lock = shared_lock_set::try_lock_file(file, LockType::Exclusive)?;
        let lock_state = LockState::new_fresh();
        let bytes = lock_state.to_bytes()?;
        lock.write_all(&bytes)?;
        lock.seek(SeekFrom::Start(0))?;
        lock.flush()?;
        Ok(Self {
            lock: Some(lock),
            lock_state,
        })
    }

    pub fn open_in(
        root_dir: &Dir,
        lock_name: &str,
        lock_type: LockType,
        block: bool,
    ) -> io::Result<Self> {
        let file = root_dir.open_with(
            lock_name,
            OpenOptions::new().create(true).write(true).read(true),
        )?;
        let file = file.into_std();
        Self::new_from_file(file, lock_type, block)
    }

    pub fn new_from_file(file: File, lock_type: LockType, block: bool) -> io::Result<Self> {
        let mut lock = if block {
            shared_lock_set::lock_file(file, lock_type)?
        } else {
            shared_lock_set::try_lock_file(file, lock_type)?
        };
        let lock_state = {
            let mut content = Vec::new();
            lock.read_to_end(&mut content)?;
            LockState::new_from_bytes(&content)?
        };
        Ok(Self {
            lock: Some(lock),
            lock_state,
        })
    }

    pub fn mark_as_written(&mut self) -> io::Result<()> {
        if !self
            .lock
            .as_ref()
            .ok_or_else(|| io_err!(Other, "Lock is invalid"))?
            .lock_type()
            .is_exclusive()
        {
            io_bail!(Other, "Cannot mark a non-exclusive lock as written");
        }

        if self.lock_state.mark_as_written() {
            let bytes = self.lock_state.to_bytes()?;
            let file: &mut File = self.inner_file();
            file.set_len(0)?;
            file.write_all(&bytes)?;
            file.seek(SeekFrom::Start(0))?;
            file.flush()?;
        }

        Ok(())
    }

    pub fn mark_as_poisoned(&mut self) -> io::Result<()> {
        if !self
            .lock
            .as_ref()
            .ok_or_else(|| io_err!(Other, "Lock is invalid"))?
            .lock_type()
            .is_exclusive()
        {
            io_bail!(Other, "Cannot mark a non-exclusive lock as poisoned");
        }

        if self.lock_state.mark_as_poisoned() {
            let bytes = self.lock_state.to_bytes()?;
            let file: &mut File = self.inner_file();
            file.set_len(0)?;
            file.write_all(&bytes)?;
            file.seek(SeekFrom::Start(0))?;
            file.flush()?;
        }

        Ok(())
    }

    pub fn upgrade(&mut self, block: bool) -> io::Result<()> {
        let old_lock = self.lock.take().expect("Lock must be valid");
        if old_lock.lock_type().is_exclusive() {
            // We already have an exclusive lock. Assign and return it.
            self.lock = Some(old_lock);
            return Ok(());
        }
        let file = old_lock.into_file();
        let mut new_lock = if block {
            shared_lock_set::lock_file(file, LockType::Exclusive)?
        } else {
            shared_lock_set::try_lock_file(file, LockType::Exclusive)?
        };

        let new_lock_state = {
            let mut content = Vec::new();

            new_lock.seek(SeekFrom::Start(0))?;
            new_lock.read_to_end(&mut content)?;
            new_lock.seek(SeekFrom::Start(0))?;
            LockState::new_from_bytes(&content)?
        };

        if new_lock_state.revision() != self.lock_state.revision() {
            drop(new_lock);
            io_bail!(Other, "Lock file was modified while upgrading lock");
        }

        self.lock_state = new_lock_state;
        self.lock = Some(new_lock);
        Ok(())
    }

    pub fn downgrade(&mut self, block: bool) -> io::Result<()> {
        let old_lock = self.lock.take().expect("Lock must be valid");
        if !old_lock.lock_type().is_exclusive() {
            // We already have a shared lock. Assign and return it.
            self.lock = Some(old_lock);
            return Ok(());
        }
        let file = old_lock.into_file();
        let mut new_lock = if block {
            shared_lock_set::lock_file(file, LockType::Shared)?
        } else {
            shared_lock_set::try_lock_file(file, LockType::Shared)?
        };

        let new_lock_state = {
            let mut content = Vec::new();
            new_lock.seek(SeekFrom::Start(0))?;
            new_lock.read_to_end(&mut content)?;
            new_lock.seek(SeekFrom::Start(0))?;
            LockState::new_from_bytes(&content)?
        };

        if new_lock_state.revision() != self.lock_state.revision() {
            drop(new_lock);
            io_bail!(Other, "Lock file was modified while upgrading lock");
        }

        self.lock_state = new_lock_state;
        self.lock = Some(new_lock);
        Ok(())
    }

    #[must_use]
    pub fn lock_type(&self) -> LockType {
        self.lock.as_ref().expect("Lock must be valid").lock_type()
    }

    #[must_use]
    pub fn is_exclusive(&self) -> bool {
        self.lock
            .as_ref()
            .is_some_and(|l| l.lock_type().is_exclusive())
    }

    #[must_use]
    pub fn is_shared(&self) -> bool {
        self.lock
            .as_ref()
            .is_some_and(|l| !l.lock_type().is_exclusive())
    }
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    use super::*;

    #[test]
    fn test_exclusive_lock_blocks() -> io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let root_dir = Dir::open_ambient_dir(temp_dir.path(), cap_std::ambient_authority())?;
        let _initial_lock = LockFile::create_in(&root_dir, "test.lock")?;
        let err = LockFile::open_in(&root_dir, "test.lock", LockType::Shared, false).unwrap_err();
        assert!(matches!(err.kind(), io::ErrorKind::WouldBlock));

        Ok(())
    }

    #[test]
    fn test_shared_lock_shares() -> io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let root_dir = Dir::open_ambient_dir(temp_dir.path(), cap_std::ambient_authority())?;
        // Create the initial exclusive lock, and drop it immediately.
        LockFile::create_in(&root_dir, "test.lock")?;
        let _shared_lock1 = LockFile::open_in(&root_dir, "test.lock", LockType::Shared, true)?;
        let _shared_lock2 = LockFile::open_in(&root_dir, "test.lock", LockType::Shared, true)?;

        Ok(())
    }

    #[test]
    fn test_upgrade_downgrade() -> io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let root_dir = Dir::open_ambient_dir(temp_dir.path(), cap_std::ambient_authority())?;
        let mut initial_lock = LockFile::create_in(&root_dir, "test.lock")?;
        assert!(LockFile::open_in(&root_dir, "test.lock", LockType::Shared, false).is_err());
        initial_lock.downgrade(true)?;
        {
            let _shared_lock = LockFile::open_in(&root_dir, "test.lock", LockType::Shared, false)?;
        }
        initial_lock.upgrade(true)?;
        assert!(LockFile::open_in(&root_dir, "test.lock", LockType::Shared, false).is_err());

        Ok(())
    }

    #[test]
    fn test_lock_state_new_fresh() {
        let state = LockState::new_fresh();
        assert_eq!(state.revision(), 0);
        assert!(!state.updated);
    }

    #[test]
    fn test_lock_state_serialization() -> io::Result<()> {
        let state = LockState::new_fresh();
        let bytes = state.to_bytes()?;

        // Deserialize
        let contents: LockContents = serde_json::from_slice(&bytes)?;
        assert_eq!(contents.version, 1);
        assert_eq!(contents.revision, 0);
        assert!(!contents.poisoned);

        Ok(())
    }

    #[test]
    fn test_lock_state_poisoned_serialization() -> io::Result<()> {
        let mut state = LockState::new_fresh();
        // Initially not poisoned
        assert!(state.mark_as_poisoned());

        // Subsequent calls do not change the state
        assert!(!state.mark_as_poisoned());
        let bytes = state.to_bytes()?;

        // Deserialize
        let contents: LockContents = serde_json::from_slice(&bytes)?;
        assert_eq!(contents.version, 1);
        assert_eq!(contents.revision, 0);
        assert!(contents.poisoned);

        Ok(())
    }

    #[test]
    fn test_lock_state_from_bytes_poisoned_fails() {
        let contents = LockContents {
            version: 1,
            revision: 5,
            poisoned: true,
        };
        let bytes = serde_json::to_vec(&contents).unwrap();

        let result = LockState::new_from_bytes(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::Other);
    }

    #[test]
    fn test_lock_state_from_bytes_wrong_version() {
        let contents = LockContents {
            version: 99,
            revision: 5,
            poisoned: false,
        };
        let bytes = serde_json::to_vec(&contents).unwrap();

        let result = LockState::new_from_bytes(&bytes);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported lock file version")
        );
    }

    #[test]
    fn test_lock_state_mark_as_written_increments() {
        let mut state = LockState::new_fresh();
        assert_eq!(state.revision(), 0);

        let incremented = state.mark_as_written();
        assert!(incremented);
        assert_eq!(state.revision(), 1);
        assert!(state.updated);
    }

    #[test]
    fn test_lock_state_mark_as_written_idempotent() {
        let mut state = LockState::new_fresh();

        // First call increments
        assert!(state.mark_as_written());
        assert_eq!(state.revision(), 1);

        // Second call does not increment
        assert!(!state.mark_as_written());
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn test_lock_state_revision_wrapping() {
        let mut state = LockState {
            revision: u32::MAX,
            updated: false,
            poisoned: false,
        };

        state.mark_as_written();
        // Should wrap to 0
        assert_eq!(state.revision(), 0);
    }
}
