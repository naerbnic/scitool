use std::sync::{Arc, Mutex};

/// A shared cell, for passing a value between threads.
#[derive(Debug)]
pub(super) struct SharedCell<T>(Arc<Mutex<Option<T>>>);

impl<T> Clone for SharedCell<T> {
    fn clone(&self) -> Self {
        SharedCell(self.0.clone())
    }
}

impl<T> SharedCell<T> {
    pub(super) fn new() -> Self {
        SharedCell(Arc::new(Mutex::new(None)))
    }

    pub(super) fn with_value(value: T) -> Self {
        SharedCell(Arc::new(Mutex::new(Some(value))))
    }

    pub(super) fn take(&self) -> Option<T>
    where
        T: Default,
    {
        std::mem::take(&mut *self.0.lock().unwrap())
    }

    pub(super) fn set_if_empty(&self, value: T) -> Result<(), T> {
        let mut guard = self.0.lock().unwrap();
        if guard.is_some() {
            return Err(value);
        }
        *guard = Some(value);
        Ok(())
    }

    pub(super) fn with_opt<F, R>(&self, body: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut guard = self.0.lock().unwrap();
        let Some(value) = guard.as_mut() else {
            return None;
        };
        Some(body(value))
    }
}
