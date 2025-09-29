/// A type that represents a value that may be "dirty", meaning it has been
/// modified locally, but not been persisted in whatever storage it came from.
#[derive(Debug, Clone)]
pub(super) struct Dirty<T> {
    /// The current value that has been persisted. This can be None if the value
    /// has never been persisted.
    current: Option<T>,

    /// The modified value, if any.
    dirty: Option<T>,
}

impl<T> Dirty<T> {
    /// Create a new `Dirty` value with a value marked as stored.
    pub(super) fn new_stored(value: T) -> Self {
        Dirty {
            current: Some(value),
            dirty: None,
        }
    }

    /// Create a new `Dirty` value with a value marked as fresh (not loaded).
    pub(super) fn new_fresh(value: T) -> Self {
        Dirty {
            current: None,
            dirty: Some(value),
        }
    }

    /// Get a reference to the current value.
    #[must_use]
    pub(super) fn get(&self) -> &T {
        self.dirty
            .as_ref()
            .or(self.current.as_ref())
            .expect("No value available")
    }

    #[expect(dead_code, reason = "Will use to check if value is stored")]
    pub(super) fn get_stored(&self) -> Option<&T> {
        self.current.as_ref()
    }

    /// Set the current value to the given value, marking it as dirty.
    pub(super) fn set(&mut self, value: T) {
        self.dirty = Some(value);
    }

    /// Returns true if the value is dirty.
    #[must_use]
    pub(super) fn is_dirty(&self) -> bool {
        self.dirty.is_some()
    }

    pub(super) fn mark_clean(&mut self) {
        if let Some(dirty) = self.dirty.take() {
            self.current = Some(dirty);
        }
    }
}

impl<T> Dirty<T>
where
    T: Clone,
{
    /// Get a clone of the current value.
    #[must_use]
    pub(super) fn get_mut(&mut self) -> &mut T {
        if self.dirty.is_none() {
            assert!(self.current.is_some());
            self.dirty = self.current.clone();
        }

        self.dirty.as_mut().expect("No value available")
    }
}
