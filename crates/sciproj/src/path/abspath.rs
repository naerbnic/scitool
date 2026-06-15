#![allow(unsafe_code)]

use std::{ffi::OsStr, fmt::Display, ops::Deref, path::Component};

use crate::path::{relpath::RelPath, segment::Segment};

fn validate_abs_path(path: &str) -> bool {
    let std_path: &std::path::Path = path.as_ref();

    // The main source of uncertainty here is that Windows has two types of
    // paths that are only partially absolute: Ones that start with a
    // drive letter only, and those that start with a backslash only. According
    // to the docs, is_absolute() considers both as not absolute.
    if !std_path.is_absolute() {
        return false;
    }

    // Ensure that all segments (aside from the roots) are normal, and match
    // the common definition of "segment" between relpath and abspath.
    for component in std_path.components() {
        match component {
            // Ignore root-like components, as those have already been checked
            // by Path::is_absolute().
            Component::RootDir | Component::Prefix(_) => {}

            // We still don't allow for . or .. segments
            Component::CurDir | Component::ParentDir => return false,
            Component::Normal(segment) => {
                if Segment::try_new(segment.to_str().unwrap()).is_none() {
                    return false;
                }
            }
        }
    }

    true
}

pub struct Ancestors<'a> {
    parent: std::path::Ancestors<'a>,
}

impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a AbsPath;

    fn next(&mut self) -> Option<Self::Item> {
        self.parent
            .next()
            .map(|p| AbsPath::from_str_unchecked(p.to_str().unwrap()))
    }
}

/// A string that represents an absolute location in the filesystem, independent
/// of base directory.
///
/// Unlike [`super::relpath::RelPath`], this does not require that it have an OS-independent
/// representation, as there are paths on different platforms that don't have
/// a common representation.
///
/// Note that this type can still suffer from TOCTOU errors.
#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[repr(transparent)]
pub struct AbsPath(str);

impl AbsPath {
    #[must_use]
    pub fn from_static(path: &'static str) -> &'static Self {
        AbsPath::from_str_unchecked(path)
    }

    #[must_use]
    pub fn from_std<P: AsRef<std::path::Path> + ?Sized>(path: &P) -> &Self {
        Self::new_opt(path.as_ref().to_str().unwrap()).unwrap()
    }

    #[must_use]
    pub fn new(path: &str) -> &Self {
        Self::new_opt(path).unwrap()
    }

    #[must_use]
    pub fn new_opt(path: &str) -> Option<&Self> {
        if validate_abs_path(path) {
            Some(AbsPath::from_str_unchecked(path))
        } else {
            None
        }
    }

    const fn from_str_unchecked(path: &str) -> &Self {
        // SAFETY: Cast is safe because str and RelPath have the same layout
        // due to repr(transparent).
        unsafe { (std::ptr::from_ref(path) as *const Self).as_ref().unwrap() }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn to_buf(&self) -> AbsPathBuf {
        AbsPathBuf(self.0.to_string())
    }

    #[must_use]
    pub fn as_std(&self) -> &std::path::Path {
        self.0.as_ref()
    }

    #[must_use]
    pub fn parent(&self) -> Option<&Self> {
        let std_path: &std::path::Path = self.0.as_ref();
        std_path
            .parent()
            .map(|p| AbsPath::from_str_unchecked(p.to_str().unwrap()))
    }

    #[must_use]
    pub fn ancestors(&self) -> Ancestors<'_> {
        let std_path: &std::path::Path = self.0.as_ref();
        Ancestors {
            parent: std_path.ancestors(),
        }
    }

    #[must_use]
    pub fn join<P: AsRef<std::path::Path>>(&self, p: P) -> AbsPathBuf {
        self.try_join(p).unwrap()
    }

    #[must_use]
    pub fn try_join<P: AsRef<std::path::Path>>(&self, p: P) -> Option<AbsPathBuf> {
        let std_path: &std::path::Path = self.0.as_ref();
        let joined = std_path.join(p.as_ref().to_str()?);
        Some(AbsPathBuf(joined.into_os_string().into_string().unwrap()))
    }

    #[must_use]
    pub fn join_rel<P: AsRef<RelPath>>(&self, rel: P) -> AbsPathBuf {
        let std_path: &std::path::Path = self.0.as_ref();
        let joined = std_path.join(rel.as_ref().as_str());
        AbsPathBuf(joined.into_os_string().into_string().unwrap())
    }

    #[must_use]
    pub fn is_dir(&self) -> bool {
        self.as_std().is_dir()
    }
}

impl Display for &AbsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<std::path::Path> for &AbsPath {
    fn as_ref(&self) -> &std::path::Path {
        self.as_std()
    }
}

impl AsRef<OsStr> for &AbsPath {
    fn as_ref(&self) -> &OsStr {
        self.as_str().as_ref()
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AbsPathBuf(String);

impl AbsPathBuf {
    #[must_use]
    pub fn new(path: String) -> Self {
        Self::try_new(path).unwrap()
    }

    #[must_use]
    pub fn try_new(path: String) -> Option<Self> {
        if validate_abs_path(&path) {
            Some(Self(path))
        } else {
            None
        }
    }

    pub fn from_std<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self::try_from_std(path).unwrap()
    }

    pub fn try_from_std<P: AsRef<std::path::Path>>(path: P) -> Option<Self> {
        Self::try_new(path.as_ref().to_str()?.to_string())
    }

    #[must_use]
    pub fn as_path(&self) -> &AbsPath {
        AbsPath::from_str_unchecked(self.0.as_str())
    }
}

impl Deref for AbsPathBuf {
    type Target = AbsPath;

    fn deref(&self) -> &Self::Target {
        AbsPath::from_str_unchecked(self.0.as_str())
    }
}

impl Display for AbsPathBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.as_path(), f)
    }
}

macro_rules! def_partial_cmp_refl {
    ($other:ty => |$path_id:ident, $other_id: ident| $pair_expr:expr) => {
        impl PartialEq<&$other> for AbsPath {
            fn eq(&self, $other_id: &&$other) -> bool {
                let $other_id = *$other_id;
                let $path_id = self;
                let (left, right) = $pair_expr;
                left == right
            }
        }

        impl PartialEq<&AbsPath> for $other {
            fn eq(&self, $path_id: &&AbsPath) -> bool {
                let $path_id = *$path_id;
                let $other_id = self;
                let (left, right) = $pair_expr;
                left == right
            }
        }

        impl PartialOrd<&AbsPath> for $other {
            fn partial_cmp(&self, $path_id: &&AbsPath) -> Option<std::cmp::Ordering> {
                let $path_id = *$path_id;
                let $other_id = self;
                let (left, right) = $pair_expr;
                PartialOrd::partial_cmp(left, right)
            }
        }

        impl PartialOrd<&$other> for AbsPath {
            fn partial_cmp(&self, $other_id: &&$other) -> Option<std::cmp::Ordering> {
                let $other_id = *$other_id;
                let $path_id = self;
                let (left, right) = $pair_expr;
                PartialOrd::partial_cmp(left, right)
            }
        }
    };
}

def_partial_cmp_refl!(str => |path, s| (path.as_str(), s));
def_partial_cmp_refl!(std::path::Path => |path, std_path| (OsStr::new(path.as_str()), std_path));
def_partial_cmp_refl!(OsStr => |path, s| (OsStr::new(path.as_str()), s));
def_partial_cmp_refl!(std::borrow::Cow<'_, str> => |path, s| (path.as_str(), s.as_ref()));
def_partial_cmp_refl!(std::borrow::Cow<'_, std::path::Path> => |path, s| (OsStr::new(path.as_str()), s.as_ref()));
def_partial_cmp_refl!(std::borrow::Cow<'_, std::ffi::OsStr> => |path, s| (OsStr::new(path.as_str()), AsRef::<OsStr>::as_ref(s)));

impl AsRef<std::path::Path> for AbsPathBuf {
    fn as_ref(&self) -> &std::path::Path {
        self.as_std()
    }
}

impl AsRef<OsStr> for AbsPathBuf {
    fn as_ref(&self) -> &OsStr {
        self.as_str().as_ref()
    }
}
