//! A library for dealing with platform-independent relative paths.
//!
//! These paths:
//!
//! - Are separated by forward slashes ('/')
//! - Are in the UTF-8 character set
//! - May not contain '..' or '.' components
//!
//! These are true for all platforms.
#![expect(unsafe_code)]

use serde::{Deserialize, de::Error as _};
use std::{
    borrow::Cow, collections::HashSet, ffi::OsStr, ops::Deref, path::Component, sync::LazyLock,
};

use itertools::{EitherOrBoth, Itertools as _};

const SPECIAL_MEANING_CHARS: &[char] = &[
    '/',  // On Windows, Unix, Mac.
    '\\', // On Windows.
    ':',  // Old mac separator, but also has special meanings as part of PATHs.
    ';',  // On Windows, part of PATHs.
    '<', '>', '|', // Shell redirection. Possible but uncommon on POSIX, forbidden on Windows.
    '*', '?', // Wildcards.
    '"', // Quote character forbidden on Windows
];

const INVALID_SEGMENTS: &[&str] = &[
    ".", "..", // Common current directory/previous directory chars.
];

static INVALID_SEGMENTS_SET: LazyLock<HashSet<&str>> =
    LazyLock::new(|| INVALID_SEGMENTS.iter().copied().collect());

fn is_normal_path_segment(segment: &str) -> bool {
    if segment.is_empty() {
        return false;
    }

    if INVALID_SEGMENTS_SET.contains(segment) {
        return false;
    }

    if segment.find(SPECIAL_MEANING_CHARS).is_some() {
        return false;
    }

    let mut components = std::path::Path::new(segment).components();

    let Some(Component::Normal(parsed_segment)) = components.next() else {
        return false;
    };

    if components.next().is_some() {
        return false;
    }

    parsed_segment == segment
}

fn validate_rel_path(path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    // Check that parsed path is the same as the split path.
    let split_iter = path.split('/');
    let sys_path: &std::path::Path = std::path::Path::new(path);
    let sys_path_components = sys_path.components();

    assert!(
        std::path::is_separator('/'),
        "The platform must support a forward slash as a separator"
    );

    for zip_item in split_iter.zip_longest(sys_path_components) {
        let EitherOrBoth::Both(split_item, component) = zip_item else {
            // Path are different lengths
            return false;
        };

        let Component::Normal(item) = component else {
            return false;
        };

        if split_item != item {
            return false;
        }

        if split_item.is_empty() {
            return false;
        }

        if split_item.find(SPECIAL_MEANING_CHARS).is_some() {
            return false;
        }
    }

    true
}

#[derive(Debug, thiserror::Error)]
#[error("prefix not found")]
pub struct StripPrefixError(());

#[derive(Debug, thiserror::Error)]
#[error("invalid relpath conversion")]
pub struct FromStdPathError(());

#[derive(Debug)]
pub struct Components<'a> {
    path: &'a str,
}

impl<'a> Iterator for Components<'a> {
    type Item = &'a Segment;

    fn next(&mut self) -> Option<Self::Item> {
        if self.path.is_empty() {
            None
        } else {
            let (seg, rest) = if let Some(slash_index) = self.path.find('/') {
                (&self.path[..slash_index], &self.path[slash_index + 1..])
            } else {
                (self.path, "")
            };
            self.path = rest;
            Some(Segment::from_str_unchecked(seg))
        }
    }
}

impl DoubleEndedIterator for Components<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.path.is_empty() {
            None
        } else {
            let (seg, rest) = if let Some(slash_index) = self.path.rfind('/') {
                (&self.path[slash_index + 1..], &self.path[..slash_index])
            } else {
                (self.path, "")
            };
            self.path = rest;
            Some(Segment::from_str_unchecked(seg))
        }
    }
}

#[derive(Debug)]
pub struct Ancestors<'a> {
    path: Option<&'a RelPath>,
}

impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a RelPath;

    fn next(&mut self) -> Option<Self::Item> {
        let path: &'a RelPath = self.path.as_ref()?;
        self.path = path.parent();
        Some(path)
    }
}

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Segment(str);

impl Segment {
    #[must_use]
    pub fn new(segment: &str) -> &Self {
        let Some(seg) = Self::try_new(segment) else {
            panic!("Invalid relative path: {segment}");
        };
        seg
    }

    #[must_use]
    pub fn try_new(segment: &str) -> Option<&Self> {
        if is_normal_path_segment(segment) {
            Some(Self::from_str_unchecked(segment))
        } else {
            None
        }
    }

    fn from_str_unchecked(path: &str) -> &Self {
        // SAFETY: Cast is safe because str and RelPath have the same layout
        // due to repr(transparent).
        unsafe { (std::ptr::from_ref(path) as *const Self).as_ref().unwrap() }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for Segment {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<RelPath> for Segment {
    fn as_ref(&self) -> &RelPath {
        RelPath::from_str_unchecked(self.as_str())
    }
}

#[repr(transparent)]
#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct RelPath(str);

impl RelPath {
    #[must_use]
    pub fn new(path: &str) -> &Self {
        let Some(rel_path) = Self::try_new(path) else {
            panic!("Invalid relative path: {path}");
        };
        rel_path
    }

    #[must_use]
    pub fn try_new(path: &str) -> Option<&Self> {
        if validate_rel_path(path) {
            Some(Self::from_str_unchecked(path))
        } else {
            None
        }
    }

    fn from_str_unchecked(path: &str) -> &Self {
        // SAFETY: Cast is safe because str and RelPath have the same layout
        // due to repr(transparent).
        unsafe { (std::ptr::from_ref(path) as *const Self).as_ref().unwrap() }
    }

    #[must_use]
    pub fn parent(&self) -> Option<&Self> {
        if self.0.is_empty() {
            return None;
        }
        let Some(slash_index) = self.0.rfind('/') else {
            return Some(Self::from_str_unchecked(""));
        };

        Some(Self::from_str_unchecked(&self.0[..slash_index]))
    }

    pub fn join(&self, rel_path: impl AsRef<RelPath>) -> RelPathBuf {
        if self.0.is_empty() {
            return RelPathBuf(rel_path.as_ref().as_str().to_string());
        }
        let mut path = self.0.to_string();
        path.push('/');
        path.push_str(rel_path.as_ref().as_str());
        RelPathBuf(path)
    }

    pub fn to_std_path(&self, root: impl AsRef<std::path::Path>) -> std::path::PathBuf {
        let root = root.as_ref();
        assert!(root.is_absolute(), "Root must be absolute");
        root.join(self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn components(&self) -> Components<'_> {
        Components { path: &self.0 }
    }

    #[must_use]
    pub fn ancestors(&self) -> Ancestors<'_> {
        Ancestors { path: Some(self) }
    }

    #[must_use]
    pub fn file_name(&self) -> Option<&'_ Segment> {
        self.components().next_back()
    }

    pub fn starts_with<P>(&self, prefix: P) -> bool
    where
        P: AsRef<RelPath>,
    {
        // Because prefix is a RelPath, it should have its contents validated.
        // We do this with a compare and a range index.
        let prefix = prefix.as_ref().as_str();
        let path = self.as_str();
        if prefix.len() > path.len() {
            return false;
        }

        if prefix != &path[..prefix.len()] {
            return false;
        }

        let rest_path = &path[prefix.len()..];

        if rest_path.is_empty() {
            return true;
        }

        rest_path.starts_with('/')
    }

    pub fn ends_with<P>(&self, prefix: P) -> bool
    where
        P: AsRef<RelPath>,
    {
        // Because prefix is a RelPath, it should have its contents validated.
        // We do this with a compare and a range index.
        let prefix = prefix.as_ref().as_str();
        let path = self.as_str();
        if prefix.len() > path.len() {
            return false;
        }

        let start_index = path.len() - prefix.len();

        if prefix != &path[start_index..] {
            return false;
        }

        let rest_path = &path[..start_index];

        if rest_path.is_empty() {
            return true;
        }

        rest_path.ends_with('/')
    }

    pub fn strip_prefix<P>(&self, prefix: P) -> Result<&RelPath, StripPrefixError>
    where
        P: AsRef<RelPath>,
    {
        let prefix: &RelPath = prefix.as_ref();
        if !self.starts_with(prefix) {
            return Err(StripPrefixError(()));
        }

        let remaining = if prefix.as_str().len() == self.as_str().len() {
            ""
        } else {
            &self.0[prefix.as_str().len() + 1..]
        };

        Ok(Self::from_str_unchecked(remaining))
    }

    #[must_use]
    pub fn to_buf(&self) -> RelPathBuf {
        RelPathBuf(self.as_str().to_string())
    }
}

macro_rules! def_partial_cmp_refl {
    ($other:ty => |$path_id:ident, $other_id: ident| $pair_expr:expr) => {
        impl PartialEq<&$other> for RelPath {
            fn eq(&self, $other_id: &&$other) -> bool {
                let $other_id = *$other_id;
                let $path_id = self;
                let (left, right) = $pair_expr;
                left == right
            }
        }

        impl PartialEq<&RelPath> for $other {
            fn eq(&self, $path_id: &&RelPath) -> bool {
                let $path_id = *$path_id;
                let $other_id = self;
                let (left, right) = $pair_expr;
                left == right
            }
        }

        impl PartialOrd<&RelPath> for $other {
            fn partial_cmp(&self, $path_id: &&RelPath) -> Option<std::cmp::Ordering> {
                let $path_id = *$path_id;
                let $other_id = self;
                let (left, right) = $pair_expr;
                PartialOrd::partial_cmp(left, right)
            }
        }

        impl PartialOrd<&$other> for RelPath {
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

impl AsRef<RelPath> for RelPath {
    fn as_ref(&self) -> &RelPath {
        self
    }
}

impl AsRef<str> for RelPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<std::path::Path> for RelPath {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(&self.0)
    }
}

impl AsRef<OsStr> for RelPath {
    fn as_ref(&self) -> &OsStr {
        OsStr::new(&self.0)
    }
}

impl<'de> Deserialize<'de> for &'de RelPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let plain_str: &'de str = Deserialize::deserialize(deserializer)?;
        RelPath::try_new(plain_str).ok_or_else(|| D::Error::custom(FromStdPathError(())))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPathBuf(String);

impl RelPathBuf {
    #[must_use]
    #[expect(single_use_lifetimes)]
    pub fn new<'a>(path: impl Into<Cow<'a, str>>) -> Self {
        Self::try_new(path).unwrap()
    }

    #[must_use]
    #[expect(single_use_lifetimes)]
    pub fn try_new<'a>(path: impl Into<Cow<'a, str>>) -> Option<Self> {
        let path = path.into().into_owned();
        if validate_rel_path(&path) {
            Some(Self(path))
        } else {
            None
        }
    }

    pub fn from_std_path<P>(path: P) -> Result<Self, FromStdPathError>
    where
        P: AsRef<std::path::Path>,
    {
        Self::from_std_path_with_opts(path, /*allow_lexical_parents=*/ false)
    }

    pub fn from_lexical_std_path<P>(path: P) -> Result<Self, FromStdPathError>
    where
        P: AsRef<std::path::Path>,
    {
        Self::from_std_path_with_opts(path, /*allow_lexical_parents=*/ true)
    }

    fn from_std_path_with_opts<P>(
        path: P,
        allow_lexical_parents: bool,
    ) -> Result<Self, FromStdPathError>
    where
        P: AsRef<std::path::Path>,
    {
        let path = path.as_ref();
        if path.is_absolute() {
            return Err(FromStdPathError(()));
        }

        let mut possible_segments = Vec::new();

        for component in path.components() {
            match component {
                Component::Normal(segment) => possible_segments.push(segment),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !allow_lexical_parents {
                        return Err(FromStdPathError(()));
                    }

                    if possible_segments.pop().is_none() {
                        // We were already at top level
                        return Err(FromStdPathError(()));
                    }
                }
                _ => {
                    // Non-relative transition.
                    return Err(FromStdPathError(()));
                }
            }
        }

        let mut validated_segments = Vec::new();

        for segment in possible_segments {
            let Some(segment_str) = segment.to_str() else {
                return Err(FromStdPathError(()));
            };

            if segment_str.find(SPECIAL_MEANING_CHARS).is_some() {
                return Err(FromStdPathError(()));
            }

            validated_segments.push(segment_str);
        }

        Ok(RelPathBuf(validated_segments.join("/")))
    }

    pub fn push<P>(&mut self, rel_path: P)
    where
        P: AsRef<RelPath>,
    {
        let rel_path = rel_path.as_ref();
        if rel_path.0.is_empty() {
            return;
        }
        self.0.push('/');
        self.0.push_str(rel_path.as_str());
    }

    #[must_use]
    pub fn as_path(&self) -> &RelPath {
        RelPath::from_str_unchecked(&self.0)
    }
}

impl Deref for RelPathBuf {
    type Target = RelPath;

    fn deref(&self) -> &Self::Target {
        RelPath::from_str_unchecked(&self.0)
    }
}

impl AsRef<RelPath> for RelPathBuf {
    fn as_ref(&self) -> &RelPath {
        RelPath::from_str_unchecked(&self.0)
    }
}

impl<'de> Deserialize<'de> for RelPathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let plain_str: String = Deserialize::deserialize(deserializer)?;
        RelPathBuf::try_new(plain_str).ok_or_else(|| D::Error::custom(FromStdPathError(())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_paths() {
        assert!(RelPath::try_new("").is_some());
        assert!(RelPath::try_new("a").is_some());
        assert!(RelPath::try_new("a/b/c").is_some());
    }

    #[test]
    fn test_invalid_paths() {
        assert!(RelPath::try_new("/a/b/c").is_none());
        assert!(RelPath::try_new("a/b/c/").is_none());
        assert!(RelPath::try_new("a/b/../c").is_none());
        assert!(RelPath::try_new("a/b/./c").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn test_invalid_windows_paths() {
        assert!(RelPath::try_new("C:/b").is_none());
        assert!(RelPath::try_new("C:a/b").is_none());
        assert!(RelPath::try_new("/a/b").is_none());
    }

    #[test]
    fn test_parent() {
        assert_eq!(RelPath::new("a/b/c").parent(), Some(RelPath::new("a/b")));
        assert_eq!(RelPath::new("a").parent(), Some(RelPath::new("")));
        assert_eq!(RelPath::new("").parent(), None);
    }
}
