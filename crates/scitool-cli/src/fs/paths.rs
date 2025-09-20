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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abs_path_try_from_path_succeeds_for_absolute() {
        let path = Path::new("/absolute/path");
        let abs_path = <&AbsPath>::try_from(path);
        assert!(abs_path.is_ok());
        assert_eq!(abs_path.unwrap().as_ref(), path);
    }

    #[test]
    fn abs_path_try_from_path_fails_for_relative() {
        let path = Path::new("relative/path");
        let abs_path = <&AbsPath>::try_from(path);
        assert!(abs_path.is_err());
    }

    #[test]
    fn abs_path_buf_try_from_path_buf_succeeds_for_absolute() {
        let path_buf = PathBuf::from("/absolute/path");
        let abs_path_buf = AbsPathBuf::try_from(path_buf);
        assert!(abs_path_buf.is_ok());
    }

    #[test]
    fn abs_path_buf_try_from_path_buf_fails_for_relative() {
        let path_buf = PathBuf::from("relative/path");
        let abs_path_buf = AbsPathBuf::try_from(path_buf);
        assert!(abs_path_buf.is_err());
    }

    #[test]
    fn abs_path_buf_push_relative() {
        let mut abs_buf = AbsPathBuf::try_from(PathBuf::from("/start")).unwrap();
        abs_buf.push("segment");
        assert_eq!(abs_buf.as_ref(), Path::new("/start/segment"));
    }

    #[test]
    fn abs_path_buf_push_absolute_replaces_path() {
        let mut abs_buf = AbsPathBuf::try_from(PathBuf::from("/start")).unwrap();
        abs_buf.push("/new/root");
        assert_eq!(abs_buf.as_ref(), Path::new("/new/root"));
    }

    #[test]
    fn abs_path_buf_pop_succeeds() {
        let mut abs_buf = AbsPathBuf::try_from(PathBuf::from("/a/b")).unwrap();
        assert!(abs_buf.pop());
        assert_eq!(abs_buf.as_ref(), Path::new("/a"));
    }

    #[test]
    #[should_panic(expected = "Popping would make path relative")]
    fn abs_path_buf_pop_panics_when_root_is_popped() {
        let mut abs_buf = AbsPathBuf::try_from(PathBuf::from("/")).unwrap();
        abs_buf.pop(); // This should panic.
    }

    #[test]
    fn abs_path_buf_set_file_name() {
        let mut abs_buf = AbsPathBuf::try_from(PathBuf::from("/a/b.txt")).unwrap();
        abs_buf.set_file_name("c.md");
        assert_eq!(abs_buf.as_ref(), Path::new("/a/c.md"));
    }

    #[test]
    fn abs_path_buf_set_extension() {
        let mut abs_buf = AbsPathBuf::try_from(PathBuf::from("/a/b.txt")).unwrap();
        assert!(abs_buf.set_extension("rs"));
        assert_eq!(abs_buf.as_ref(), Path::new("/a/b.rs"));
    }

    #[test]
    fn deref_allows_path_methods() {
        let abs_buf = AbsPathBuf::try_from(PathBuf::from("/a/b.txt")).unwrap();
        assert_eq!(abs_buf.file_name().unwrap(), "b.txt");
    }

    #[test]
    fn into_path_buf_works() {
        let path_buf = PathBuf::from("/a/b");
        let abs_buf = AbsPathBuf::try_from(path_buf.clone()).unwrap();
        let new_path_buf: PathBuf = abs_buf.into();
        assert_eq!(new_path_buf, path_buf);
    }
}
