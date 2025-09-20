#![allow(unsafe_code, reason = "Needed for unsized wrappers")]

use std::{
    collections::TryReserveError,
    ops::Deref,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
#[error("Path is not absolute")]
pub struct AbsPathConvertError;

#[repr(transparent)]
pub struct AbsPath(Path);

impl AbsPath {
    unsafe fn cast_from_path(path: &Path) -> &Self {
        // SAFETY: Caller must ensure that path is absolute.
        unsafe { &*(std::ptr::from_ref(path) as *const AbsPath) }
    }
}

impl<'a> TryFrom<&'a Path> for &'a AbsPath {
    type Error = AbsPathConvertError;

    fn try_from(value: &'a Path) -> Result<Self, Self::Error> {
        if value.is_absolute() {
            Ok(unsafe { AbsPath::cast_from_path(value) })
        } else {
            Err(AbsPathConvertError)
        }
    }
}

impl Deref for AbsPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Path> for AbsPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

pub struct AbsPathBuf(PathBuf);

impl AbsPathBuf {
    // These are versions of PathBuf's methods that preserve the invariant
    // that the path is absolute.
    pub fn push<P: AsRef<Path>>(&mut self, path: P) {
        self.0.push(path);
    }

    pub fn set_file_name<S: AsRef<std::ffi::OsStr>>(&mut self, file_name: S) {
        self.0.set_file_name(file_name);
    }

    pub fn set_extension<S: AsRef<std::ffi::OsStr>>(&mut self, extension: S) -> bool {
        self.0.set_extension(extension)
    }

    pub fn pop(&mut self) -> bool {
        // Note: This can make the path relative if we pop too many times.
        assert!(
            self.0.parent().is_some_and(Path::is_absolute),
            "Popping would make path relative"
        );
        self.0.pop()
    }

    #[must_use]
    pub fn as_abs_path(&self) -> &AbsPath {
        unsafe { AbsPath::cast_from_path(&self.0) }
    }

    #[must_use]
    pub fn leak<'a>(self) -> &'a AbsPath {
        Box::leak(Box::new(self)).as_abs_path()
    }

    #[must_use]
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }

    #[must_use]
    pub fn into_os_string(self) -> std::ffi::OsString {
        self.0.into_os_string()
    }

    #[must_use]
    pub fn into_boxed_path(self) -> Box<AbsPath> {
        let raw = Box::into_raw(self.0.into_boxed_path()) as *mut AbsPath;
        // SAFETY:
        // - We ensured the invariant when creating self.
        // - AbsPath is #[repr(transparent)] over Path, so they can be directly cast.
        unsafe { Box::from_raw(raw) }
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve(additional)
    }

    pub fn reserve_exact(&mut self, additional: usize) {
        self.0.reserve_exact(additional);
    }

    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve_exact(additional)
    }

    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }

    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.0.shrink_to(min_capacity);
    }
}

// This is fine, because it loosens the invariant.
impl From<AbsPathBuf> for PathBuf {
    fn from(value: AbsPathBuf) -> Self {
        value.0
    }
}

impl TryFrom<PathBuf> for AbsPathBuf {
    type Error = AbsPathConvertError;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        if value.is_absolute() {
            Ok(Self(value))
        } else {
            Err(AbsPathConvertError)
        }
    }
}

impl Deref for AbsPathBuf {
    type Target = AbsPath;
    fn deref(&self) -> &Self::Target {
        self.as_abs_path()
    }
}
