# OS Lock Contention Detection

## Context

In order to provide accurate file locking both within process threads and
between processes, we have two layers of locking: One in process that ensures
the platform only locks existing files, and the OS advisory file locks.
Acquiring the first lock in process requires locking the file, and the last one
released will unlock it.

## Problem

Both the in-process and OS locks use FIFO locking in order to provide lock
fairness, but those systems do not work together. If the in-process locks get
a constant stream of shared lockers such that the file lock is never released,
it will block the exclusive lockers from other processes.

If instead we were able to detect if there was contention on the lock (even if
imperfect/racy), we would be able to detect circumstances where we needed to
have shared lockers wait, and when we could opportunistically give threads
locks on the current file lock without releasing the file lock each time.

There is no cross-platform way of detecting contention on an advisory file lock
while at the same time holding the lock.

## Solution

We define a locking protocol using byte-range advisory locks on ranges in the
first page of the lock to both indicate interest in taking the file lock, and
ways of then detecting if there exists a process waiting on the current lock.

### OS Assumptions

- With an open read-write file handle, a process is able to lock any byte range
  in the open file.
- Byte-range locks are handled at process-level; Threads within the process all
  share the same lock state for a file handle.
- Locks are handled in an FIFO-like manner; When a process is waiting for an
  exclusive lock, further blocking shared lock attempts are blocked until the
  exclusive lock is taken and released.
- Byte-range locks that do not intersect are independent; There are in effect
  separate FIFO queues for each non-overlapping byte range.

### OS Non-Assumptions

- We do not assume that file locking is re-entrant. Calling lock multiple times
  on the same file handle (with or without the same lock type) is unspecified.
- We do not assume that byte ranges that exist entirely outside the range of the
  file are lockable. If this is so for a platform, creation of the file must
  add enough bytes (arbitrary in general, but zeros by convention) to allow for
  independent locks on the two first bytes.
- We do not assume that byte range locks can be directly upgraded or downgraded.

### Protocol

We define two byte ranges in the lock file:

- Bytes [0-1): The preparation lock (Pre)
- Bytes [1-2): The primary lock (Prim)

All locking involves lock operations on these two locks.

For all following operations:

- Any lock has a lock type (either shared (SH) or exclusive (EX))
- While trying to take a lock type, or holding on to a lock, all lock operations
  use that lock type.
- For any non-lock related failure (e.g. filesystem error, etc.) the operation
  will release all held locks and report the error.

Invariants for the following operations:

- The Pre lock is only held while waiting for a lock. Outside of the
  operations, either only the Prim lock is held, or no lock is held.
- The Prim lock can only be taken while holding the Pre lock.

#### `OpenFile()`

1. Open the given file to be created, in read/write mode, and not truncated.
2. If the platform does not allow for byte locks on the first two bytes for an
   empty file, set the file length to at least 2.
   - This data may be arbitrary, and can be potentially meaningful for the
     processes accessing it. The actual contents are not used.
3. Return the file handle.

#### `Lock()`

Precondition: No lock of any type is held on the file.

To take a blocking lock (either shared (SH) or exclusive (EX)):

Note: All lock steps use the the desired SH or EX lock type.

1. Take a lock on the Pre lock. (blocks until taken)
2. Take a lock on the Prim lock. (blocks until taken)
3. Release the Pre lock.
4. Report success.

Postcondition: A lock is held on the Prim lock of the file.

#### `TryLock()`

Precondition: No lock of any type is held on the file.

To take a non-blocking lock:

1. Attempt to take a lock on the Pre lock. Fail with WOULD_BLOCK if it would
   block.
2. Attempt to take a lock on the Prim lock.
   1. If the lock attempt fails, release the Pre lock, and fail with
      WOULD_BLOCK.
3. Release the Pre lock.
4. Report success.

Postconditions:

- On all failures (including WOULD_BLOCK) no lock is held.
- On success, the lock is held.

#### `Release()`

Precondition: A lock of any type is held on the file (thus Prim is locked).

To release a lock:

1. Release the Prim lock.
2. Report success.

Postcondition: No lock is held.

#### `IsContended()`

Precondition: A lock of any type (of type T) is held.

To detect lock contention while holding the Prim lock:

1. Attempt to take a lock of type T on the Pre lock.
   1. If the lock fails, return CONTENTION.
2. Release the Pre lock.
3. Report NO_CONTENTION.

Postconditions:

- The Prim lock is held, still of type T.
- If CONTENTION is returned, there exists a process that is blocked on our lock.
- If NO_CONTENTION is returned, at the time of check, there was no process that
  had established interest in waiting on the lock.

### Analysis

#### Lock Operation Properties

We need to make sure this follows the normally accepted properties of `Lock()`,
`TryLock()`, and `Release()`.

1. Uncontensted SH locks will take the lock at the same time

   If all lockers are using SH locks, then all locks on Pre and Prim will
   succeed if there are no waiting EX locks. Thus they will always succeed.

2. SH locks block EX locks, EX locks block SH/EX locks

   By the underlying OS byte-lock system, these are invariant.

3. Locking is as fair as the underlying OS implementation of byte-range locking.

   If there is an EX `Lock()` operation held by process A, it will be
   holding onto the Pre lock. When process B tries to run the SH `Lock()`
   operation, it will be blocked on the Pre lock as the OS prevents SH locks
   from holding a lock at the same time as an EX lock.

   Further lock waiting is handled by the OS, so reduces to its behavior.

4. Considering only this protocol, deadlocks with `Lock()` are impossible.

   Deadlock on locks can only occur if there is a loop of blocking lock attempts
   such that each process in the loop is holding onto a lock that another
   process in the loop is waiting for. We can prevent deadlock loops by
   defining an absolute order of locks, and with the invariant that a blocking
   lock can never be taken by a process that is holding a lock larger lock.

   We define the order of locks such that the Pre lock is smaller than the Prim
   lock. From this, we observe that we never have a time that we hold the Prim
   lock, and take a blocking lock on the Pre lock. Thus deadlocks are
   impossible.

5. `IsContended()` returns CONTENTION only if there is another process that
   is trying to take the lock (either blocking or non-blocking).

   In order for the attempt to lock Prim to fail, there must exist another
   process which is following the `Lock()` operation, or another process that
   briefly held the lock in the `TryLock()` operation. In both of these cases,
   there was another process that (however briefly) was interested in taking the lock, indicating some amount of contention.

   Note that this cannot be caused by the lock briefly held by `IsContended()`, as that requires that the lock to be held as a precondition. Since we only
   share locks with processes with a compatible lock type, these locks will not
   block if no other processes are involved.

6. `IsContended()` returns NO_CONTENTION if there is an ordering of lock events
   that would allow the lock to be taken without contention.

   When we are able to take the Pre lock in `IsContended()`, the only way that
   it could have succeed is if all other locks that are being acquired are
   compatible with the currently held lock, or that there is an interleaving
   where the another process is scheduled after our test lock attempt of Pre.

   If another process runs `Lock()` with an incompatible lock type, and we are
   guaranteed that our call to `IsContended()` will occur in the window where
   that process is holding the Pre lock, then we must detect contention.

## Conclusion

This protocol allows us to be able to test the contention of a currently held
OS file lock without significant false positives/negatives, while still
behaving like normal non-byte-range file locking. With these tools, in-process
locks can detect at critical points (e.g. when locks are acquired/released) if
there is contention, and prevent holding on to a lock that would cause
starvation of other processes.
