//! Atomic directory implementation.

mod commit;
mod dir_lock;
mod new_engine;
mod recovery;
mod types;
mod util;

use std::{
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use tempfile::TempDir;

pub use self::types::{DirEntry, FileType, Metadata};
pub use crate::fs::ops::WriteMode;
use crate::fs::{
    atomic_dir::{
        commit::CommitFileData,
        dir_lock::DirLock,
        recovery::{check_needs_recovery, recover_exclusive},
        util::create_old_path,
    },
    err_helpers::io_bail,
    file_lock::LockType,
};

struct Inner {}

struct ReadOnlyHandle {}

struct OpenOptions {}

/// A high-level atomic directory that allows for changing files within
/// a directory atomically.
///
/// This guarantees that either all changes are applied, or none are, even in
/// the case of crashes or interruptions. Changes are staged in a temporary
/// directory and then committed atomically. If the program crashes, a recovery
/// process will attempt to complete the commit the next time an `AtomicDir`
/// is created for the same directory.
pub struct AtomicDir {
    inner: Option<Inner>,
}

impl AtomicDir {
}

/// Helper functions
impl AtomicDir {
    /// A convenience method to write data to a file within the transaction.
    ///
    /// This is equivalent to using `open_options` to open a file for writing
    /// and then writing the data.
    pub fn write<P>(&self, path: &P, write_mode: &WriteMode, data: &[u8]) -> io::Result<()>
    where
        P: AsRef<Path> + ?Sized,
    {
        todo!()
    }

    /// A convenience method to read data from a file within the transaction.
    ///
    /// This reads the entire contents of the file into a `Vec<u8>`.
    pub async fn read<P>(&self, path: &P) -> io::Result<Vec<u8>>
    where
        P: AsRef<Path> + ?Sized,
    {
        todo!()
    }
}

impl Drop for AtomicDir {
    fn drop(&mut self) {
        todo!()
    }
}

/// A builder to create a new `AtomicDir`, or overwrite an existing one.
pub struct NewDirBuilder {
    target_lock: DirLock,
    temp_path: TempDir,
}

impl NewDirBuilder {
    pub fn new<P>(path: &P) -> io::Result<Self>
    where
        P: AsRef<Path> + ?Sized,
    {
        let target_lock = DirLock::acquire(path.as_ref(), LockType::Exclusive)?;
        // We should have nothing at the target path, or at the commit file for the path. Otherwise,
        // we might be overwriting an existing directory.
        if std::fs::exists(target_lock.path())? {
            io_bail!(
                AlreadyExists,
                "Target path already exists: {}",
                target_lock.path().display()
            );
        }

        if check_needs_recovery(&target_lock)? {
            io_bail!(
                AlreadyExists,
                "Target path has an incomplete commit, so it's not empty: {}",
                target_lock.path().display()
            );
        }

        // Create a temporary directory within the parent of the target path.
        let temp_path = tempfile::TempDir::new_in(target_lock.parent())?;
        Ok(NewDirBuilder {
            target_lock,
            temp_path,
        })
    }

    pub fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let path = util::normalize_path(path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(self.temp_path.path().join(parent))?;
        }
        // We don't need to worry about atomicity here, since the entire temp directory
        // will be moved into place atomically during commit, or deleted during abort.
        {
            let mut file = std::fs::File::options()
                .write(true)
                .create_new(true)
                .open(self.target_lock.path().join(&path))?;
            file.write_all(data)?;
            file.sync_all()?;
        }
        Ok(())
    }

    pub fn commit(self) -> io::Result<()> {
        let commit = CommitFileData::new(
            PathBuf::from(
                self.temp_path
                    .path()
                    .file_name()
                    .expect("Temp dir has a name")
                    .to_owned(),
            ),
            create_old_path(&self.target_lock).into_path_buf(),
        );
        // We want to persist the temp directory, so we will be durable once the
        // commit file is written.
        let temp_path = self.temp_path.keep();
        if let Err(e) = commit.commit_file(&self.target_lock) {
            // Failed to write the commit file. Try to clean up after ourselves.
            drop(std::fs::remove_dir_all(&temp_path));
            return Err(e);
        }

        // Now, perform the recovery steps to move the temp directory into place.
        //
        // Even if this fails, opening the directory again will recover it.
        recover_exclusive(&self.target_lock)?;

        Ok(())
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::fs::{
//         atomic_dir::schema::{CommitEntry, CommitSchema, DeleteEntry, OverwriteEntry},
//         paths::RelPathBuf,
//     };

//     use super::*;
//     use tempfile::tempdir;

//     #[test]
//     fn test_write_and_commit() -> io::Result<()> {
//         let dir = tempdir()?;

//         let atomic_dir = AtomicDir::new_at_dir(dir.path())?;
//         atomic_dir.write("foo.txt", &WriteMode::CreateNew, b"hello")?;

//         atomic_dir.commit()?;

//         let contents = std::fs::read_to_string(dir.path().join("foo.txt"))?;
//         assert_eq!(contents, "hello");

//         Ok(())
//     }

//     #[test]
//     fn test_delete_and_commit() -> io::Result<()> {
//         let dir = tempdir()?;

//         // Create a file to be deleted.
//         std::fs::write(dir.path().join("foo.txt"), "hello")?;

//         let atomic_dir = AtomicDir::new_at_dir(dir.path())?;
//         atomic_dir.delete(Path::new("foo.txt"))?;
//         atomic_dir.commit()?;

//         assert!(!dir.path().join("foo.txt").exists());

//         Ok(())
//     }

//     #[test]
//     fn test_write_delete_and_commit() -> io::Result<()> {
//         let dir = tempdir()?;

//         // Create a file to be overwritten.
//         std::fs::write(dir.path().join("foo.txt"), "old content")?;

//         let atomic_dir = AtomicDir::new_at_dir(dir.path())?;
//         atomic_dir.write(Path::new("foo.txt"), &WriteMode::Overwrite, b"new content")?;
//         atomic_dir.delete(Path::new("bar.txt"))?;
//         atomic_dir.write(Path::new("bar.txt"), &WriteMode::CreateNew, b"new file")?;
//         atomic_dir.commit()?;

//         let contents = std::fs::read_to_string(dir.path().join("foo.txt"))?;
//         assert_eq!(contents, "new content");
//         let contents_bar = std::fs::read_to_string(dir.path().join("bar.txt"))?;
//         assert_eq!(contents_bar, "new file");

//         Ok(())
//     }

//     #[test]
//     fn test_list_dir_reflects_staged_state() -> io::Result<()> {
//         let dir = tempdir()?;
//         let assets_dir = dir.path().join("assets");
//         std::fs::create_dir(&assets_dir)?;

//         std::fs::write(assets_dir.join("existing.txt"), b"old")?;
//         std::fs::write(assets_dir.join("deleted.txt"), b"remove me")?;
//         std::fs::write(assets_dir.join("untouched.txt"), b"keep")?;

//         let atomic_dir = AtomicDir::new_at_dir(dir.path())?;

//         atomic_dir.write(Path::new("assets/new.txt"), &WriteMode::CreateNew, b"new")?;
//         atomic_dir.write(
//             Path::new("assets/existing.txt"),
//             &WriteMode::Overwrite,
//             b"updated",
//         )?;
//         atomic_dir.delete(Path::new("assets/deleted.txt"))?;

//         let entries = atomic_dir.list_dir(Path::new("assets"))?;
//         let mut observed = std::collections::BTreeSet::new();

//         for entry in entries {
//             let entry = entry?;
//             assert!(entry.file_type().is_file());
//             let rel = entry.file_name().to_string_lossy().into_owned();
//             observed.insert(rel);
//         }

//         let expected: std::collections::BTreeSet<String> =
//             ["existing.txt", "new.txt", "untouched.txt"]
//                 .into_iter()
//                 .map(String::from)
//                 .collect();

//         assert_eq!(observed, expected);

//         Ok(())
//     }

//     #[test]
//     fn test_recovery() -> io::Result<()> {
//         let dir = tempdir()?;

//         // Simulate a partial commit.
//         let commit_schema = CommitSchema::new(
//             RelPathBuf::new_checked("tmpdir-recovery-test").unwrap(),
//             vec![
//                 CommitEntry::Overwrite(OverwriteEntry::new(
//                     RelPathBuf::new_checked("foo.txt").unwrap(),
//                 )),
//                 CommitEntry::Delete(DeleteEntry::new(
//                     RelPathBuf::new_checked("bar.txt").unwrap(),
//                 )),
//             ],
//         );

//         // Create the temporary directory and file.
//         std::fs::create_dir(dir.path().join("tmpdir-recovery-test"))?;
//         std::fs::write(dir.path().join("tmpdir-recovery-test/foo.txt"), "recovered")?;

//         // Create a file to be deleted.
//         std::fs::write(dir.path().join("bar.txt"), "to be deleted")?;

//         // Write the commit file.
//         let commit_data = serde_json::to_vec(&commit_schema)?;
//         std::fs::write(dir.path().join(COMMIT_PATH), commit_data)?;

//         // Now, run recovery.
//         let _atomic_dir = AtomicDir::new_at_dir(dir.path())?;

//         // Check that recovery happened.
//         let contents = std::fs::read_to_string(dir.path().join("foo.txt"))?;
//         assert_eq!(contents, "recovered");
//         assert!(!dir.path().join("bar.txt").exists());
//         assert!(!dir.path().join(COMMIT_PATH).exists());
//         assert!(!dir.path().join("tmpdir-recovery-test").exists());

//         Ok(())
//     }

//     #[test]
//     fn test_dir_locking() -> io::Result<()> {
//         let dir = tempdir()?;

//         {
//             // Acquire a lock by creating an AtomicDirInner.
//             let _atomic_dir1 = AtomicDir::new_at_dir(dir.path())?;

//             // Try to acquire another lock on the same directory.
//             // This should fail with `Ok(None)` because the first one is still held.
//             let atomic_dir2 = AtomicDir::try_new_at_dir(dir.path())?;
//             assert!(
//                 atomic_dir2.is_none(),
//                 "Should not be able to acquire lock while it's held"
//             );
//         } // atomic_dir1 is dropped here, releasing the lock.

//         // Now that the first lock is released, we should be able to acquire a new one.
//         let atomic_dir3 = AtomicDir::try_new_at_dir(dir.path())?;
//         assert!(
//             atomic_dir3.is_some(),
//             "Should be able to acquire lock after it's released"
//         );

//         Ok(())
//     }
// }
