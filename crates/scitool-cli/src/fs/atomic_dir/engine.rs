use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Component, Path, PathBuf},
};

use futures::{StreamExt as _, TryStream, TryStreamExt as _};
use rand::{Rng, distr::Alphanumeric};
use tokio::{io::AsyncWriteExt as _, sync::Mutex};

use crate::fs::{
    atomic_dir::{
        COMMIT_PATH, LOCK_PATH,
        recovery::recover_path,
        schema::{CommitEntry, CommitSchema, DeleteEntry, OverwriteEntry},
        types::{DirEntry, FileType},
        util::{is_valid_path, write_file_atomic},
    },
    err_helpers::{io_bail, io_err_map},
    ops::{FileSystemOperations, LockFile, OpenOptionsFlags, PathKind, WriteMode},
    paths::{AbsPath, AbsPathBuf, RelPath, RelPathBuf},
};

struct ChildEntry {
    name: RelPathBuf,
    file_type: FileType,
}

fn get_child_of_descendant(base: &RelPath, descendant: &RelPath) -> Option<ChildEntry> {
    let stripped_path = descendant.strip_prefix(base).ok()?;
    let mut components = stripped_path.components();
    match components.next() {
        Some(Component::Normal(os_str)) => {
            let name = RelPath::new_checked(os_str).expect("Normal component is always valid");
            let file_type = if components.next().is_some() {
                FileType::new_of_dir()
            } else {
                FileType::new_of_file()
            };
            Some(ChildEntry {
                name: name.into(),
                file_type,
            })
        }
        _ => None, // The stripped path is empty or starts with something unexpected.
    }
}

fn normalize_path(path: &Path, temp_dir: &Path) -> io::Result<RelPathBuf> {
    let mut rel_path = RelPathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                io_bail!(
                    Other,
                    "Package file path must be strictly relative: {}",
                    path.display()
                );
            }
            Component::ParentDir => {
                if !rel_path.pop() {
                    io_bail!(
                        Other,
                        "Path must not contain a directory upreference before the start: {}",
                        path.display()
                    );
                }
            }
            Component::CurDir => { /* Skip */ }
            Component::Normal(os_str) => {
                rel_path
                    .push(RelPath::new_checked(os_str).expect("Normal component is always valid"));
            }
        }
    }

    if rel_path.as_os_str().is_empty() {
        io_bail!(Other, "Destination path cannot be empty");
    }

    is_valid_path(&rel_path, temp_dir)?;

    Ok(rel_path)
}

async fn create_temp_dir<FS, R>(fs: &FS, rng: &mut R, base_dir: &Path) -> io::Result<RelPathBuf>
where
    FS: FileSystemOperations,
    R: Rng,
{
    for _ in 0..10 {
        let rand_str = rng
            .sample_iter(&Alphanumeric)
            .map(char::from)
            .take(16)
            .collect::<String>();

        let dir_name: PathBuf = format!("tmpdir-{rand_str}").into();
        let dir_name: RelPathBuf = dir_name.try_into().map_err(io_err_map!(
            Other,
            "Generated temporary directory name is not a valid relative path"
        ))?;
        let possible_temp_dir = base_dir.join(&dir_name);
        match fs.create_dir(&possible_temp_dir).await {
            Ok(()) => return Ok(dir_name),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                // Try again with a different name.
            }
            Err(err) => return Err(err),
        }
    }
    io_bail!(Other, "Failed to create a unique temporary directory");
}

struct DirLock<LF>
where
    LF: LockFile + Send,
{
    _lock: LF,
}

impl<LF> DirLock<LF>
where
    LF: LockFile + Send,
{
    pub(crate) async fn acquire<FS>(fs: &FS, dir_root: &AbsPath) -> io::Result<Self>
    where
        FS: FileSystemOperations<FileLock = LF>,
    {
        let lock_path = dir_root.join_rel(RelPath::new_checked(LOCK_PATH).unwrap());
        let lock = fs.open_lock_file(&lock_path).await?;
        lock.lock_exclusive().await?;
        Ok(DirLock { _lock: lock })
    }
    pub(crate) async fn try_acquire<FS>(fs: &FS, dir_root: &AbsPath) -> io::Result<Option<Self>>
    where
        FS: FileSystemOperations<FileLock = LF>,
    {
        let lock_path = dir_root.join_rel(RelPath::new_checked(LOCK_PATH).unwrap());
        let lock = fs.open_lock_file(&lock_path).await?;
        if !lock.try_lock_exclusive().await? {
            return Ok(None);
        }
        Ok(Some(DirLock { _lock: lock }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum TempFileStatus {
    /// The path is unchanged from the original file state.
    Unchanged,
    /// The path has been written to the temporary directory.
    Written,
    /// The path has been deleted from the final directory.
    Deleted,
}

struct AtomicDirState {
    file_statuses: BTreeMap<RelPathBuf, TempFileStatus>,
    /// True if the state of the atomic dir has been resolved, either by
    /// committing or aborting.
    completed: bool,
}

pub(super) struct Engine<FS: FileSystemOperations + 'static> {
    /// The file system operations implementation to use.
    fs: FS,

    /// A lock that ensures exclusive access to the directory.
    dir_lock: Option<DirLock<FS::FileLock>>,

    /// The root directory being managed.
    dir_root: AbsPathBuf,

    /// The temporary directory inside the root directory.
    temp_dir: RelPathBuf,

    state: Mutex<AtomicDirState>,
}

impl<FS> Engine<FS>
where
    FS: FileSystemOperations,
{
    fn relative_temp_file_path(&self, relative_path: &Path) -> io::Result<RelPathBuf> {
        let relative_path = is_valid_path(relative_path, &self.temp_dir)?;
        Ok(self.temp_dir.join_rel(relative_path))
    }

    fn normalize_path(&self, path: &Path) -> io::Result<RelPathBuf> {
        normalize_path(path, &self.temp_dir)
    }

    async fn create_at_dir_with_lock(
        fs: FS,
        dir_root: AbsPathBuf,
        dir_lock: DirLock<FS::FileLock>,
    ) -> io::Result<Self> {
        // It's possible that the previous operation was interrupted, so we
        // should try to recover the directory first.
        recover_path(&fs, &dir_root).await?;

        let temp_dir = create_temp_dir(&fs, &mut rand::rng(), &dir_root).await?;

        Ok(Engine {
            fs,
            dir_lock: Some(dir_lock),
            dir_root,
            temp_dir,
            state: Mutex::new(AtomicDirState {
                file_statuses: BTreeMap::new(),
                completed: false,
            }),
        })
    }

    pub(super) async fn create_at_dir(fs: FS, dir_root: &Path) -> io::Result<Self> {
        let mut curr_dir = AbsPathBuf::new_checked(&std::env::current_dir()?)
            .map_err(io_err_map!(Other, "Failed to get current directory"))?;

        curr_dir.push(dir_root);
        let dir_root = curr_dir;
        let dir_lock = DirLock::acquire(&fs, &dir_root).await?;
        Self::create_at_dir_with_lock(fs, dir_root, dir_lock).await
    }

    pub(super) async fn try_create_at_dir(fs: FS, dir_root: &Path) -> io::Result<Option<Self>> {
        let mut curr_dir = AbsPathBuf::new_checked(&std::env::current_dir()?)
            .map_err(io_err_map!(Other, "Failed to get current directory"))?;

        curr_dir.push(dir_root);
        let dir_root = curr_dir;
        let Some(dir_lock) = DirLock::try_acquire(&fs, &dir_root).await? else {
            return Ok(None);
        };
        Ok(Some(
            Self::create_at_dir_with_lock(fs, dir_root, dir_lock).await?,
        ))
    }

    pub(super) async fn delete_path(&self, path: &Path) -> io::Result<()> {
        let rel_target_path = self.normalize_path(path)?;
        let rel_temp_path = self.relative_temp_file_path(&rel_target_path)?;
        let abs_target_path = self.dir_root.join_rel(&rel_target_path);
        let abs_temp_path = self.dir_root.join_rel(&rel_temp_path);

        let mut state_guard = self.state.lock().await;

        let file_status = state_guard
            .file_statuses
            .entry(rel_target_path.clone())
            .or_insert(TempFileStatus::Unchanged);

        match *file_status {
            TempFileStatus::Deleted => {
                // The file has already been deleted, no changes needed.
            }
            TempFileStatus::Unchanged => {
                match self.fs.get_path_kind(&abs_target_path).await? {
                    Some(PathKind::Directory) => io_bail!(IsADirectory, "Path is a directory"),
                    Some(PathKind::Other) => io_bail!(Other, "Path is not a regular file"),
                    Some(PathKind::File) => {}
                    None => {
                        // The file does not exist. Avoid adding a delete entry
                        // to keep things clean.
                        return Ok(());
                    }
                }
            }
            TempFileStatus::Written => {
                // The file has been written to the temporary directory, so we
                // can just remove it from there.
                self.fs.remove_file(&abs_temp_path).await?;
            }
        }

        *file_status = TempFileStatus::Deleted;

        Ok(())
    }

    pub(super) async fn open_file(
        &self,
        path: &Path,
        options: &OpenOptionsFlags,
    ) -> io::Result<FS::File> {
        let rel_target_path = self.normalize_path(path)?;
        let rel_target_parent = rel_target_path.parent_rel().unwrap_or_default();
        let abs_temp_root = self.dir_root.join_rel(&self.temp_dir);
        let abs_target_path = self.dir_root.join_rel(&rel_target_path);
        let abs_temp_path = abs_temp_root.join_rel(&rel_target_path);
        let abs_temp_parent = abs_temp_root.join_rel(rel_target_parent);

        let mut file_status_guard = self.state.lock().await;
        let file_status_guard = &mut *file_status_guard;
        let file_state_entry = file_status_guard
            .file_statuses
            .entry(rel_target_path.clone())
            .or_insert(TempFileStatus::Unchanged);
        match *file_state_entry {
            TempFileStatus::Written => {
                // The file has already been written to the temporary directory,
                // so we can open it directly from there.
                self.fs.open_file(&abs_temp_path, options).await
            }

            TempFileStatus::Deleted => {
                // The file has been deleted, so if we're creating it, we can
                // open it in the temporary directory. Otherwise, we should
                // return an error.
                if !options.can_create_file() {
                    io_bail!(
                        NotFound,
                        "File has been deleted: {}",
                        rel_target_path.display()
                    );
                }
                self.fs.create_dir_all(&abs_temp_parent).await?;

                let file = self.fs.open_file(&abs_temp_path, options).await?;
                *file_state_entry = TempFileStatus::Written;
                Ok(file)
            }

            TempFileStatus::Unchanged => {
                // We have not touched this file yet, so we need to set up its state.
                // We require that if there are any changes to a file, the data must be
                // only changed in the temp directory. This should be as transparent as
                // possible to the user.
                if !options.can_change_file() {
                    // We are not going to change the file, so we can open it directly.
                    return self.fs.open_file(&abs_target_path, options).await;
                }

                self.fs.create_dir_all(&abs_temp_parent).await?;

                {
                    let (should_create, should_copy) =
                        match self.fs.get_path_kind(&abs_target_path).await? {
                            Some(PathKind::Directory) => {
                                io_bail!(IsADirectory, "Path is a directory")
                            }
                            Some(PathKind::Other) => io_bail!(Other, "Path is not a regular file"),
                            Some(PathKind::File) => (true, options.uses_original_data()),
                            None => (false, false),
                        };

                    if should_create {
                        // Create an empty file in the temporary directory.
                        let mut target_flags = OpenOptionsFlags::default();
                        target_flags.set_write(true);
                        target_flags.set_create_new(true);
                        let mut target_file =
                            self.fs.open_file(&abs_temp_path, &target_flags).await?;
                        if should_copy {
                            // Copy the file to the temporary directory if we are going
                            // to change it.
                            let mut source_flags = OpenOptionsFlags::default();
                            source_flags.set_read(true);
                            let mut source_file =
                                self.fs.open_file(&abs_target_path, &source_flags).await?;

                            tokio::io::copy(&mut source_file, &mut target_file).await?;
                        }
                    }
                }

                let file = self.fs.open_file(&abs_temp_path, options).await?;
                *file_state_entry = TempFileStatus::Written;
                Ok(file)
            }
        }
    }

    pub(super) async fn list_dir<'a>(
        &'a self,
        path: &Path,
    ) -> io::Result<impl TryStream<Ok = DirEntry, Error = io::Error> + Unpin + 'a> {
        let rel_target_path = self.normalize_path(path)?;
        let abs_target_path = self.dir_root.join_rel(&rel_target_path);

        // First loop through the temporary directory, yielding all entries.
        //
        // Keep track of all the entries we have seen so far, so we can avoid
        // yielding duplicates from the main directory.
        let mut seen_entries = BTreeSet::new();

        let mut temp_path_entries = Vec::new();
        {
            let locked_state = self.state.lock().await;
            for (path, status) in &locked_state.file_statuses {
                let Some(child) = get_child_of_descendant(&rel_target_path, path) else {
                    continue;
                };
                match status {
                    TempFileStatus::Deleted => {
                        // We have deleted this entry, so we should not yield it.
                        //
                        // Note that we only want to do this with files, as the
                        // target directory may still have other entries in it.
                        if child.file_type.is_file() {
                            seen_entries.insert(child.name.clone());
                        }
                    }
                    TempFileStatus::Unchanged => {
                        // We have not changed this entry, so we will yield it
                        // from the main directory listing.
                    }
                    TempFileStatus::Written => {
                        if !seen_entries.insert(child.name.clone()) {
                            // We have already yielded this entry.
                            continue;
                        }
                        temp_path_entries.push(child);
                    }
                }
            }
        };
        let target_path_entry_stream = self
            .fs
            .list_dir(&abs_target_path)
            .await
            .map(Some)
            .or_else(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .map_err(io_err_map!(
                Other,
                "Failed to read existing directory entries"
            ))?;
        let target_path_entries = if let Some(stream) = target_path_entry_stream {
            stream
                .try_filter_map(async |entry| {
                    if seen_entries.contains(entry.file_name()) {
                        return Ok(None);
                    }
                    Ok(Some(ChildEntry {
                        name: RelPathBuf::new_checked(entry.file_name()).map_err(io_err_map!(
                            Other,
                            "Entry name is not a valid relative path"
                        ))?,
                        file_type: match entry.file_type() {
                            PathKind::Directory => FileType::new_of_dir(),
                            PathKind::File => FileType::new_of_file(),
                            PathKind::Other => {
                                // We skip non-regular files.
                                return Ok(None);
                            }
                        },
                    }))
                })
                .try_collect()
                .await?
        } else {
            // The target directory does not exist. We will only yield entries
            // from the temporary directory.
            Vec::new()
        };
        // Combine the two lists, sort them, and yield them.
        let mut path_entries = temp_path_entries;
        path_entries.extend(target_path_entries);
        path_entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(futures::stream::iter(path_entries)
            .map(move |entry| {
                DirEntry::new(
                    rel_target_path.clone(),
                    entry.name.into_os_string(),
                    entry.file_type,
                )
            })
            .map(Ok))
    }

    pub(super) async fn exists(&self, path: &Path) -> io::Result<bool> {
        let rel_target_path = self.normalize_path(path)?;
        let abs_target_path = self.dir_root.join_rel(&rel_target_path);
        let abs_temp_path = self
            .dir_root
            .join_rel(&self.temp_dir)
            .join_rel(&rel_target_path);

        let locked_state = self.state.lock().await;
        match locked_state.file_statuses.get(&rel_target_path) {
            Some(TempFileStatus::Deleted) => Ok(false),
            Some(TempFileStatus::Written) => {
                Ok(self.fs.get_path_kind(&abs_temp_path).await?.is_some())
            }
            Some(TempFileStatus::Unchanged) | None => {
                // We have not changed this file, so check the main directory.
                Ok(self.fs.get_path_kind(&abs_target_path).await?.is_some())
            }
        }
    }

    pub(super) async fn abort(mut self) -> io::Result<()> {
        // We release the dir lock before we wait on an async operation to immediately
        // allow other operations to proceed.
        std::mem::drop(self.dir_lock.take());
        // Remove the temporary directory.
        let abs_temp_dir = self.dir_root.join_rel(&self.temp_dir);
        self.fs.remove_dir_all(&abs_temp_dir).await
    }

    pub(super) async fn commit(mut self) -> io::Result<()> {
        let state = self.state.get_mut();
        let pending_commits = std::mem::take(&mut state.file_statuses)
            .into_iter()
            .filter_map(|(path, status)| match status {
                TempFileStatus::Unchanged => None,
                TempFileStatus::Written => Some(CommitEntry::Overwrite(OverwriteEntry::new(path))),
                TempFileStatus::Deleted => Some(CommitEntry::Delete(DeleteEntry::new(path))),
            })
            .collect::<Vec<_>>();
        if pending_commits.is_empty() {
            // Nothing to commit.
            return Ok(());
        }

        let commit_schema = CommitSchema::new(self.temp_dir.clone(), pending_commits);
        let commit_data = serde_json::to_vec(&commit_schema)
            .map_err(io_err_map!(Other, "Failed to serialize commit schema"))?;

        write_file_atomic(
            &self.fs,
            &self.dir_root,
            &self.dir_root.join_rel(&self.temp_dir),
            WriteMode::CreateNew,
            RelPath::new_checked(COMMIT_PATH).map_err(io_err_map!(
                Other,
                "Failed to create relative path for commit file"
            ))?,
            async |mut file| {
                file.write_all(&commit_data).await?;
                Ok(())
            },
        )
        .await?;

        // Now that we have written the commit file, we can perform recovery
        // to finalize the changes.
        recover_path(&self.fs, &self.dir_root).await?;

        Ok(())
    }
}

impl<FS> Drop for Engine<FS>
where
    FS: FileSystemOperations,
{
    fn drop(&mut self) {
        let state = self.state.get_mut();
        if !state.completed {
            // We have not been committed or aborted, so we should abort the transaction.
            // We do this in a background task to avoid blocking the drop.
            let dir_root = self.dir_root.clone();
            let temp_dir = self.temp_dir.clone();
            let abs_temp_dir = dir_root.join_rel(&temp_dir);
            tokio::spawn(self.fs.remove_dir_all(&abs_temp_dir));
        }
    }
}
