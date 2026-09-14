//! Runtime support for Task.

use std::cell::{Cell, OnceCell};

#[derive(Debug, Clone)]
pub struct TaskResultSlot<T> {
    data: OnceCell<T>,
    init_started: Cell<bool>,
}

impl<T> TaskResultSlot<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: OnceCell::new(),
            init_started: Cell::new(false),
        }
    }

    pub fn get_or_try_init<E, F>(&self, init: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if let Some(data) = self.data.get() {
            return Ok(data);
        }
        assert!(
            !self.init_started.replace(true),
            "Reentrant access to task slot"
        );

        match init() {
            Ok(val) => {
                self.data.set(val).ok().expect("Should never be set here.");
                Ok(self.data.get().unwrap())
            }
            Err(err) => {
                self.init_started.set(false);
                Err(err)
            }
        }
    }

    pub fn get_or_init<F>(&self, init: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if let Some(data) = self.data.get() {
            return data;
        }
        assert!(
            !self.init_started.replace(true),
            "Reentrant access to task slot"
        );

        self.data.get_or_init(init)
    }
}

/// Helper trait to extract the type and error for a result type.
pub trait ResultParams {
    type Out;
    type Err;
}

impl<T, E> ResultParams for Result<T, E> {
    type Out = T;
    type Err = E;
}

pub type ResultOut<R> = <R as ResultParams>::Out;
pub type ResultErr<R> = <R as ResultParams>::Err;
pub type RefOutResult<'a, R> = std::result::Result<&'a ResultOut<R>, ResultErr<R>>;

impl<T> Default for TaskResultSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}
