#![allow(unsafe_code)]

use std::{collections::HashSet, ops::Deref, path::Component, sync::LazyLock};

pub(super) const SPECIAL_MEANING_CHARS: &[char] = &[
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

    pub(super) fn from_str_unchecked(path: &str) -> &Self {
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

impl AsRef<std::path::Path> for Segment {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(self.as_str())
    }
}
