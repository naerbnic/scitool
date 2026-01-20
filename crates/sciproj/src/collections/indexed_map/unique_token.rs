use std::{cmp::Ordering, fmt::Debug, hash::Hash, sync::Arc};

/// A unique token which is able to be cloned and used as a key elsewhere.
///
/// Two tokens are guaranteed to be equal if and only if they were created
/// from the same call to `UniqueToken::new`.
pub(super) struct UniqueToken {
    // We use a ZST here, because we only need the pointer to be unique.
    // Because the Arc itself has its own header, the raw pointer will be
    // different, even if in general the raw pointer to a ZST may be shared
    // with an adjacent type.
    ptr: Arc<()>,
}

impl Debug for UniqueToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("UniqueToken")
            .field(&Arc::as_ptr(&self.ptr))
            .finish()
    }
}

impl Clone for UniqueToken {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr.clone(),
        }
    }
}

impl PartialEq for UniqueToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ptr, &other.ptr)
    }
}

impl Eq for UniqueToken {}

impl PartialOrd for UniqueToken {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UniqueToken {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ptr().cmp(&other.as_ptr())
    }
}

impl Hash for UniqueToken {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ptr().hash(state);
    }
}

impl UniqueToken {
    /// Creates a new unique token.
    ///
    /// This value can be cloned and used as a key. Two tokens created
    /// separately from this call are guaranteed to be different.
    pub(super) fn new() -> Self {
        Self { ptr: Arc::new(()) }
    }

    /// Helper to return the plain pointer value. Safe, as we only use this
    /// value for comparison.
    fn as_ptr(&self) -> *const () {
        Arc::as_ptr(&self.ptr)
    }
}
