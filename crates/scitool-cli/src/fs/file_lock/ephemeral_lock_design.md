# Ephemeral File Lock Design

## Purpose

This document describes an ephemeral file locking protocol designed for low-contention, cross-platform synchronization during critical moments in the atomic_dir lifecycle. The lock file serves as a signal that a directory is being edited, even when the directory itself may momentarily not exist during the commit process.

## Design Goals

1. **Self-cleaning**: Lock files are automatically removed when locks are released, minimizing persistent state
2. **Cross-platform**: Works correctly on Unix (using flock) and Windows (using LockFileEx)
3. **Low-contention**: Optimized for scenarios where lock conflicts are rare
4. **Crash-resilient**: While not perfect, the protocol handles most crash scenarios gracefully
5. **Stateless**: Lock files contain no data; the lock itself is the only state

## Key Properties

- Lock files are always empty (zero bytes)
- Lock files are typically cleaned up automatically on release
- Supports both shared and exclusive locks
- Uses platform file locking primitives (flock/LockFileEx)
- Uses inode/file index comparison for identity verification

## Protocol Details

### Lock Acquisition

The acquisition process uses a retry loop to handle race conditions:

```
loop {
    1. Open file with O_CREAT | O_RDWR | O_TRUNC
       - Creates file if it doesn't exist
       - Truncates to zero if it does exist (safe since files are always empty)
       - Returns file descriptor fd1
    
    2. Attempt to acquire lock on fd1
       - For exclusive: try_lock_exclusive(fd1, blocking/non-blocking)
       - For shared: try_lock_shared(fd1, blocking/non-blocking)
       - If non-blocking and would block, return error immediately
    
    3. Verify file identity using same_file check:
       a. Open the file path again WITHOUT O_CREAT: fd2 = open(path, O_RDONLY)
       b. Compare fd1 and fd2 using same_file (checks inode on Unix, file index on Windows)
       c. If same_file(fd1, fd2) returns true:
          - Close fd2
          - Return success with Lock { fd1, lock_type, path }
       d. If same_file fails or files differ:
          - File was deleted/replaced between steps 1 and 3
          - Close both fd1 and fd2
          - Retry from step 1
}
```

### Lock Release

The release process differs based on lock type and attempts opportunistic cleanup:

#### Exclusive Lock Release

```
1. Unlink the lock file from the filesystem
   - Safe because we hold exclusive lock
   - No other process can have the file open for locking
2. Close the file descriptor
   - This releases the lock
```

#### Shared Lock Release

```
1. Close the file descriptor
   - This releases our shared lock
   - Other shared lock holders may still have the file open

2. Attempt opportunistic cleanup:
   a. Try to open the file path WITHOUT O_CREAT: fd = open(path, O_RDWR)
      - May fail if file was already deleted (expected)
   b. If open succeeds, try to acquire exclusive lock (NON-BLOCKING):
      - try_lock_exclusive(fd, NON_BLOCKING)
   c. If exclusive lock succeeds:
      - We are now the only locker (or there are no lockers)
      - Verify with same_file that we locked the right file
      - Unlink the file
      - Close fd
   d. If exclusive lock fails or open fails:
      - Another process holds a lock and will handle cleanup
      - Close fd if opened
```

### Why This Works

#### Correctness Properties

1. **Exclusive lock uniqueness**: At most one process can hold an exclusive lock on a file
   - Platform guarantees: flock(LOCK_EX) and LockFileEx(LOCKFILE_EXCLUSIVE_LOCK)

2. **Multiple shared locks**: Multiple processes can hold shared locks simultaneously
   - Platform guarantees: flock(LOCK_SH) and LockFileEx(0)

3. **File identity verification prevents TOCTOU**:
   - The same_file check after locking ensures we locked the file we intended
   - If file was deleted between open and lock, the reopen will either:
     - Fail (file doesn't exist) → retry
     - Open a different file (different inode) → same_file fails → retry
   - No process proceeds with a lock on the wrong file

4. **Cleanup correctness**:
   - Exclusive lock holder always cleans up (it's the only locker)
   - Shared lock holders use try_lock_exclusive to determine cleanup responsibility
   - Exactly one process will succeed in getting the exclusive lock → exactly one cleanup
   - If no process gets exclusive lock, a live lock holder exists → they'll clean up later

#### O_TRUNC Safety

Using O_TRUNC is safe because:

- Lock files are always empty by design (zero bytes)
- Truncating an empty file is a no-op
- No state is stored in the file itself
- The lock is the only state that matters

#### Race Condition Handling

**Race: File deleted between open and lock**

- Detection: same_file check fails when reopening
- Resolution: Retry loop creates/locks new file

**Race: Multiple processes trying to clean up**

- Resolution: try_lock_exclusive (non-blocking) ensures exactly one succeeds
- Others see EWOULDBLOCK and abort cleanup

**Race: Process crashes while holding lock**

- Impact: Lock file persists (leaked)
- Mitigation: Next process to acquire and release will clean it up
- This is acceptable for low-contention scenarios

## Cross-Platform Considerations

### Unix (Linux, macOS, BSD)

- Uses `flock()` system call
- Advisory locks (processes can ignore them)
- Locks tied to file descriptors
- Locks released on fd close or process exit
- `same_file` uses inode comparison

### Windows

- Uses `LockFileEx()` / `UnlockFileEx()`
- Mandatory locks (enforced by OS)
- Different lock inheritance behavior with child processes
- Locks released on handle close or process exit
- `same_file` uses file index comparison

### Rust Abstraction

- Use `fs2` crate or similar for cross-platform lock API
- Use `same_file` crate for identity comparison
- Both handle platform differences transparently

## Use Cases

This ephemeral locking scheme is ideal for:

1. **Atomic directory operations**: Protecting critical sections during directory commits
2. **Low-contention locks**: Where conflicts are rare but coordination is essential
3. **Transient coordination**: When the lock's existence signals an operation in progress
4. **No state needed**: When the lock itself is sufficient state

This scheme is NOT ideal for:

1. **High-contention scenarios**: Lock file churn could be problematic
2. **Long-held locks**: Increases risk of crash-induced lock file leaks
3. **Distributed filesystems**: Advisory locks may not work correctly (e.g., NFS)
4. **State persistence**: If you need to store data alongside the lock

## Integration with atomic_dir

The ephemeral lock will be used as a "directory edit lock" placed alongside the target directory. Key aspects:

1. **Predictable location**: Lock file path derived from directory path (e.g., `dir.lock`)
2. **Guards critical operations**: Held during directory rename/replace operations
3. **Survives directory absence**: Lock file exists even when directory is temporarily absent
4. **Combined with commit file**: Durability from commit file + exclusivity from lock = robust recovery
5. **Low overhead**: Clean up happens automatically in typical cases

## Implementation Notes

### Retry Strategy

- Infinite retry loop for the identity check race
- Should be extremely rare in practice (window is microseconds)
- Could add max retry count if concerned about infinite loops

### Blocking vs Non-blocking

- Support both modes in the API
- Non-blocking useful for try-lock semantics
- Blocking useful for guaranteed acquisition

### Error Handling

- Distinguish between "lock held" and "I/O error"
- Return appropriate error types for each case
- Log/warn on unusual conditions (e.g., many retries)

### Testing Considerations

- Race conditions are timing-dependent and hard to test
- Consider stress testing with multiple processes
- Test cleanup behavior with various lock type combinations
- Test crash recovery (kill process holding lock, verify next lock cleans up)

## Future Enhancements

Possible improvements if needed:

1. **Timeout support**: Add configurable timeout for blocking locks
2. **Fairness improvements**: Priority queue for waiters to prevent starvation
3. **Lock file metadata**: Add minimal metadata (PID) for debugging while keeping cleanup simple
4. **Exponential backoff**: On retry loop to reduce contention
5. **Monitoring/metrics**: Track lock acquisition times, retry counts, cleanup success rates

## References

- POSIX flock(2) man page
- Windows LockFileEx documentation
- `same_file` crate: <https://docs.rs/same-file/>
- `fs2` crate: <https://docs.rs/fs2/> (or `fs4` for newer API)
