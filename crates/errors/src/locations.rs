use std::{fmt, panic::Location};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceLoc(&'static Location<'static>);

impl SourceLoc {
    #[track_caller]
    pub(crate) fn current() -> Self {
        Self(Location::caller())
    }

    #[cfg(test)]
    pub(crate) fn file(&self) -> &str {
        self.0.file()
    }
}

impl fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

// Keep the same Debug impl as Location.
impl fmt::Debug for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
