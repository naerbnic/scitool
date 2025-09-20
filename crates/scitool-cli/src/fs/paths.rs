#![allow(unsafe_code, reason = "Needed for unsized wrappers")]

use std::{
    borrow::{Borrow, Cow},
    collections::TryReserveError,
    ops::Deref,
    path::{Path, PathBuf},
};

macro_rules! define_path_wrapper {
    (
        $(#[$path_meta:meta])*
        $path_wrapper:ident,
        $(#[$path_buf_meta:meta])*
        $path_buf_wrapper:ident,
        $(#[$error_meta:meta])*
        $error_type:ident,
        $convert_error_message:literal,
        $invariant_check:expr
    ) => {
        $(#[$error_meta])*
        #[derive(Debug, thiserror::Error)]
        #[error($convert_error_message)]
        pub struct $error_type;

        $(#[$path_meta])*
        #[derive(Debug)]
        #[repr(transparent)]
        pub struct $path_wrapper(Path);

        impl $path_wrapper {
            /// # Safety
            ///
            /// Caller must ensure that `path` satisfies the invariant.
            unsafe fn cast_from_path(path: &Path) -> &Self {
                // SAFETY: The wrapper is #[repr(transparent)] over Path.
                unsafe { &*(std::ptr::from_ref(path) as *const $path_wrapper) }
            }

            fn to_path_buf_wrapper(&self) -> $path_buf_wrapper {
                $path_buf_wrapper(self.0.to_path_buf())
            }

            pub fn new_checked<P>(path: &P) -> Result<&Self, $error_type> where P: AsRef<Path> + ?Sized {
                let path = path.as_ref();
                if $invariant_check(path) {
                    Ok(unsafe { Self::cast_from_path(path) })
                } else {
                    Err($error_type)
                }
            }
        }

        impl<'a> TryFrom<&'a Path> for &'a $path_wrapper {
            type Error = $error_type;

            fn try_from(value: &'a Path) -> Result<Self, Self::Error> {
                $path_wrapper::new_checked(value)
            }
        }

        impl Deref for $path_wrapper {
            type Target = Path;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl AsRef<Path> for $path_wrapper {
            fn as_ref(&self) -> &Path {
                &self.0
            }
        }

        impl ToOwned for $path_wrapper {
            type Owned = $path_buf_wrapper;

            fn to_owned(&self) -> Self::Owned {
                self.to_path_buf_wrapper()
            }
        }

        impl serde::Serialize for $path_wrapper {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for &'de $path_wrapper {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let path: &Path = serde::Deserialize::deserialize(deserializer)?;
                <$path_wrapper>::new_checked(path).map_err(serde::de::Error::custom)
            }
        }

        $(#[$path_buf_meta])*
        #[derive(Debug, Clone)]
        pub struct $path_buf_wrapper(PathBuf);

        impl $path_buf_wrapper {
            // Not public to allow for named_accessors.
            fn as_path_wrapper(&self) -> & $path_wrapper {
                unsafe { $path_wrapper::cast_from_path(&self.0) }
            }

            pub fn new_checked<P>(path: &P) -> Result<Self, $error_type> where P: AsRef<Path> + ?Sized{
                Ok(<$path_wrapper>::new_checked(path)?.to_path_buf_wrapper())
            }

            // All methods are those that cannot violate invariants.
            #[must_use]
            pub fn as_path(&self) -> &Path {
                unsafe { $path_wrapper::cast_from_path(&self.0) }
            }

            #[must_use]
            pub fn leak<'a>(self) -> &'a $path_wrapper {
                Box::leak(Box::new(self)).as_path_wrapper()
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
            pub fn into_boxed_path(self) -> Box<$path_wrapper> {
                let raw = Box::into_raw(self.0.into_boxed_path()) as *mut $path_wrapper;
                // SAFETY:
                // - We ensured the invariant when creating self.
                // - The path wrapper is #[repr(transparent)] over Path.
                unsafe { Box::from_raw(raw) }
            }

            // Methods from PathBuf that are safe to expose
            #[must_use]
            pub fn capacity(&self) -> usize { self.0.capacity() }
            pub fn reserve(&mut self, additional: usize) { self.0.reserve(additional); }
            pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> { self.0.try_reserve(additional) }
            pub fn reserve_exact(&mut self, additional: usize) { self.0.reserve_exact(additional); }
            pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> { self.0.try_reserve_exact(additional) }
            pub fn shrink_to_fit(&mut self) { self.0.shrink_to_fit(); }
            pub fn shrink_to(&mut self, min_capacity: usize) { self.0.shrink_to(min_capacity); }
        }

        impl From<$path_buf_wrapper> for PathBuf {
            fn from(value: $path_buf_wrapper) -> Self {
                value.0
            }
        }

        impl TryFrom<PathBuf> for $path_buf_wrapper {
            type Error = $error_type;

            fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
                if $invariant_check(&value) {
                    Ok(Self(value))
                } else {
                    Err($error_type)
                }
            }
        }

        impl Deref for $path_buf_wrapper {
            type Target = $path_wrapper;
            fn deref(&self) -> &Self::Target {
                self.as_path_wrapper()
            }
        }

        impl AsRef<$path_wrapper> for $path_buf_wrapper {
            fn as_ref(&self) -> &$path_wrapper {
                self.as_path_wrapper()
            }
        }

        impl AsRef<Path> for $path_buf_wrapper {
            fn as_ref(&self) -> &Path {
                &self.0
            }
        }

        impl Borrow<$path_wrapper> for $path_buf_wrapper {
            fn borrow(&self) -> &$path_wrapper {
                self.as_path_wrapper()
            }
        }

        impl From<$path_buf_wrapper> for Cow<'_, $path_wrapper> {
            fn from(value: $path_buf_wrapper) -> Self {
                Cow::Owned(value)
            }
        }

        impl serde::Serialize for $path_buf_wrapper {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $path_buf_wrapper {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let path: PathBuf = serde::Deserialize::deserialize(deserializer)?;
                <$path_buf_wrapper>::try_from(path).map_err(serde::de::Error::custom)
            }
        }
    }
}

define_path_wrapper! {
    /// A wrapper around `Path` that guarantees the path is absolute.
    AbsPath,
    /// A wrapper around `PathBuf` that guarantees the path is absolute.
    AbsPathBuf,
    /// Error returned when trying to convert a `Path` or `PathBuf` to an absolute path wrapper, but the path is not absolute.
    AbsPathConvertError,
    "Path is not absolute",
    Path::is_absolute
}

impl AbsPath {
    #[must_use]
    pub fn join_rel(&self, path: &RelPath) -> AbsPathBuf {
        // It should be impossible to join an absolute path with a relative path and get a relative path.
        // This is asserted by the invariant of AbsPathBuf.
        self.0
            .join(path)
            .try_into()
            .expect("Joining absolute and relative paths should yield an absolute path")
    }

    #[must_use]
    pub fn to_abs_path_buf(&self) -> AbsPathBuf {
        self.to_path_buf_wrapper()
    }
}

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
        self.as_path_wrapper()
    }
}

fn is_relative_path(path: &Path) -> bool {
    // This is a bit more complicated that simply `!path.is_absolute()` because
    // we want to reject prefixes as well. All components must be normal, '..', or '.'.
    //
    // If we allow prefixes, then on Windows a path like `C:` would be considered relative,
    // which refers to the current directory on the C drive, which is not what we want.
    path.components().all(|c| {
        matches!(
            c,
            std::path::Component::Normal(_)
                | std::path::Component::CurDir
                | std::path::Component::ParentDir
        )
    })
}

define_path_wrapper! {
    /// A wrapper around `Path` that guarantees the path is relative.
    RelPath,
    /// A wrapper around `PathBuf` that guarantees the path is relative.
    RelPathBuf,
    /// Error returned when trying to convert a `Path` or `PathBuf` to a relative path wrapper, but the path is not relative.
    RelPathConvertError,
    "Path is not relative",
    is_relative_path
}

impl RelPath {
    #[must_use]
    pub fn join_rel(&self, path: &RelPath) -> RelPathBuf {
        // It should be impossible to join two relative paths and get an absolute path.
        // This is asserted by the invariant of RelPathBuf.
        self.0
            .join(path)
            .try_into()
            .expect("Joining two relative paths should yield a relative path")
    }
}

impl RelPathBuf {
    // These are versions of PathBuf's methods that preserve the invariant
    // that the path is relative.

    pub fn push(&mut self, path: &RelPath) {
        self.0.push(path);
        assert!(is_relative_path(self), "Pushing made path absolute");
    }

    pub fn set_file_name<S: AsRef<std::ffi::OsStr>>(&mut self, file_name: S) {
        self.0.set_file_name(file_name);
    }

    pub fn set_extension<S: AsRef<std::ffi::OsStr>>(&mut self, extension: S) -> bool {
        self.0.set_extension(extension)
    }

    pub fn pop(&mut self) -> bool {
        // Even the empty relative path is valid, so popping is always allowed.
        self.0.pop()
    }

    #[must_use]
    pub fn as_rel_path(&self) -> &RelPath {
        unsafe { RelPath::cast_from_path(&self.0) }
    }
}

// Discriminatiors for arbitrary Path objects.

#[derive(Debug, thiserror::Error)]
#[error("Path is neither absolute nor relative.")]
pub struct ClassifyError;

pub enum PathKind<'a> {
    Abs(&'a AbsPath),
    Rel(&'a RelPath),
}

impl<'a> PathKind<'a> {
    #[must_use]
    pub fn as_abs(&self) -> Option<&'a AbsPath> {
        match self {
            PathKind::Abs(abs) => Some(abs),
            PathKind::Rel(_) => None,
        }
    }

    #[must_use]
    pub fn as_rel(&self) -> Option<&'a RelPath> {
        match self {
            PathKind::Abs(_) => None,
            PathKind::Rel(rel) => Some(rel),
        }
    }
}

pub fn classify_path(path: &'_ Path) -> Result<PathKind<'_>, ClassifyError> {
    if let Ok(abs) = AbsPath::new_checked(path) {
        Ok(PathKind::Abs(abs))
    } else if let Ok(rel) = RelPath::new_checked(path) {
        Ok(PathKind::Rel(rel))
    } else {
        Err(ClassifyError)
    }
}

pub enum PathBufKind {
    Abs(AbsPathBuf),
    Rel(RelPathBuf),
}

impl PathBufKind {
    #[must_use]
    pub fn as_abs(self) -> Option<AbsPathBuf> {
        match self {
            PathBufKind::Abs(abs) => Some(abs),
            PathBufKind::Rel(_) => None,
        }
    }

    #[must_use]
    pub fn as_rel(self) -> Option<RelPathBuf> {
        match self {
            PathBufKind::Abs(_) => None,
            PathBufKind::Rel(rel) => Some(rel),
        }
    }
}

pub fn classify_path_buf(path: PathBuf) -> Result<PathBufKind, ClassifyError> {
    if let Ok(abs) = AbsPathBuf::try_from(path.clone()) {
        Ok(PathBufKind::Abs(abs))
    } else if let Ok(rel) = RelPathBuf::try_from(path) {
        Ok(PathBufKind::Rel(rel))
    } else {
        Err(ClassifyError)
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
        let mut abs_buf = AbsPathBuf::new_checked("/start").unwrap();
        abs_buf.push("segment");
        assert_eq!(abs_buf.as_path(), Path::new("/start/segment"));
    }

    #[test]
    fn abs_path_buf_push_absolute_replaces_path() {
        let mut abs_buf = AbsPathBuf::new_checked("/start").unwrap();
        abs_buf.push("/new/root");
        assert_eq!(abs_buf.as_path(), Path::new("/new/root"));
    }

    #[test]
    fn abs_path_buf_pop_succeeds() {
        let mut abs_buf = AbsPathBuf::new_checked("/a/b").unwrap();
        assert!(abs_buf.pop());
        assert_eq!(abs_buf.as_path(), Path::new("/a"));
    }

    #[test]
    #[should_panic(expected = "Popping would make path relative")]
    fn abs_path_buf_pop_panics_when_root_is_popped() {
        let mut abs_buf = AbsPathBuf::new_checked("/").unwrap();
        abs_buf.pop(); // This should panic.
    }

    #[test]
    fn abs_path_buf_set_file_name() {
        let mut abs_buf = AbsPathBuf::new_checked("/a/b.txt").unwrap();
        abs_buf.set_file_name("c.md");
        assert_eq!(abs_buf.as_path(), Path::new("/a/c.md"));
    }

    #[test]
    fn abs_path_buf_set_extension() {
        let mut abs_buf = AbsPathBuf::new_checked("/a/b.txt").unwrap();
        assert!(abs_buf.set_extension("rs"));
        assert_eq!(abs_buf.as_path(), Path::new("/a/b.rs"));
    }

    #[test]
    fn deref_allows_path_methods() {
        let abs_buf = AbsPathBuf::new_checked("/a/b.txt").unwrap();
        assert_eq!(abs_buf.file_name().unwrap(), "b.txt");
    }

    #[test]
    fn into_path_buf_works() {
        let path_buf = PathBuf::from("/a/b");
        let abs_buf = AbsPathBuf::try_from(path_buf.clone()).unwrap();
        let new_path_buf: PathBuf = abs_buf.into();
        assert_eq!(new_path_buf, path_buf);
    }

    // --- RelPath and RelPathBuf Tests ---

    #[test]
    fn rel_path_try_from_path_succeeds_for_relative() {
        let path = Path::new("relative/path");
        let rel_path = <&RelPath>::try_from(path);
        assert!(rel_path.is_ok());
        assert_eq!(rel_path.unwrap().as_ref(), path);
    }

    #[test]
    fn rel_path_try_from_path_fails_for_absolute() {
        let path = Path::new("/absolute/path");
        let rel_path = <&RelPath>::try_from(path);
        assert!(rel_path.is_err());
    }

    #[test]
    fn rel_path_buf_try_from_path_buf_succeeds_for_relative() {
        let path_buf = PathBuf::from("relative/path");
        let rel_path_buf = RelPathBuf::try_from(path_buf);
        assert!(rel_path_buf.is_ok());
    }

    #[test]
    fn rel_path_buf_try_from_path_buf_fails_for_absolute() {
        let path_buf = PathBuf::from("/absolute/path");
        let rel_path_buf = RelPathBuf::try_from(path_buf);
        assert!(rel_path_buf.is_err());
    }

    #[test]
    fn rel_path_buf_push_relative() {
        let mut rel_buf = RelPathBuf::new_checked("start").unwrap();
        let segment = RelPath::new_checked("segment").unwrap();
        rel_buf.push(segment);
        assert_eq!(rel_buf.as_path(), Path::new("start/segment"));
    }

    #[test]
    fn rel_path_buf_pop_succeeds() {
        let mut rel_buf = RelPathBuf::new_checked("a/b").unwrap();
        assert!(rel_buf.pop());
        assert_eq!(rel_buf.as_path(), Path::new("a"));
        assert!(rel_buf.pop());
        assert_eq!(rel_buf.as_path(), Path::new(""));
        assert!(!rel_buf.pop());
    }

    #[test]
    fn rel_path_join_rel() {
        let rel_path = RelPath::new_checked("a/b").unwrap();
        let other_rel = RelPath::new_checked("c/d").unwrap();
        let joined = rel_path.join_rel(other_rel);
        assert_eq!(joined.as_path(), Path::new("a/b/c/d"));
    }

    #[test]
    fn abs_path_join_rel() {
        let abs_path = AbsPath::new_checked("/a/b").unwrap();
        let rel_path = RelPath::new_checked("c/d").unwrap();
        let joined = abs_path.join_rel(rel_path);
        assert_eq!(joined.as_path(), Path::new("/a/b/c/d"));
    }

    // --- Classification Tests ---

    #[test]
    fn classify_path_abs() {
        let path = Path::new("/a/b");
        let classified = classify_path(path).unwrap();
        match classified {
            PathKind::Abs(abs) => assert_eq!(abs.as_ref(), path),
            PathKind::Rel(_) => panic!("Classified absolute path as relative"),
        }
    }

    #[test]
    fn classify_path_rel() {
        let path = Path::new("a/b");
        let classified = classify_path(path).unwrap();
        match classified {
            PathKind::Abs(_) => panic!("Classified relative path as absolute"),
            PathKind::Rel(rel) => assert_eq!(rel.as_ref(), path),
        }
    }

    #[test]
    fn classify_path_buf_abs() {
        let path_buf = PathBuf::from("/a/b");
        let classified = classify_path_buf(path_buf.clone()).unwrap();
        match classified {
            PathBufKind::Abs(abs) => assert_eq!(abs.as_path(), path_buf.as_path()),
            PathBufKind::Rel(_) => panic!("Classified absolute path as relative"),
        }
    }

    #[test]
    fn classify_path_buf_rel() {
        let path_buf = PathBuf::from("a/b");
        let classified = classify_path_buf(path_buf.clone()).unwrap();
        match classified {
            PathBufKind::Abs(_) => panic!("Classified relative path as absolute"),
            PathBufKind::Rel(rel) => assert_eq!(rel.as_path(), path_buf.as_path()),
        }
    }
}
