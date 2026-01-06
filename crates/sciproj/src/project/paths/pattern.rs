use itertools::Itertools;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    str::FromStr,
};
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

use crate::{
    helpers,
    project::paths::{matcher::PathMatcher, regex::Node},
};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("An empty string is not a valid pattern")]
    EmptyPattern,
    #[error("A pattern segment (between path separators) cannot be empty")]
    EmptySegment,
    #[error("A pattern cannot end with a '/'")]
    EmptySuffix,
    #[error("Malformed pattern: {0}")]
    MalformedPattern(String),
    #[error("A placeholder name must not be empty")]
    EmptyPlaceholder,
}

impl ParseError {
    #[expect(single_use_lifetimes, reason = "False positive; Removing causes error")]
    fn malformed<'a>(s: impl Into<Cow<'a, str>>) -> Self {
        Self::MalformedPattern(s.into().into_owned())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Failed to merge patterns")]
pub enum MergeError {
    #[error("The patterns have different placeholders")]
    DifferentPlaceholders,
}

fn next_char(s: &mut &str) -> Option<char> {
    match s.chars().next() {
        Some(c) => {
            *s = &s[c.len_utf8()..];
            Some(c)
        }
        None => None,
    }
}

fn is_valid_literal_char(c: char) -> bool {
    match c {
        // Allowed literal punctuation.
        //
        // We keep this minimal to avoid any Unicode characters that can be
        // misinterpreted or confused at a glance.
        '-' | '_' | '.' => true,
        _ => matches!(
            c.general_category_group(),
            GeneralCategoryGroup::Letter
                | GeneralCategoryGroup::Number
                | GeneralCategoryGroup::Mark
        ),
    }
}

fn validate_placeholder_name(name: &str) -> Result<(), ParseError> {
    let mut chars = name.chars();
    let Some(first_char) = chars.next() else {
        return Err(ParseError::EmptyPlaceholder);
    };

    if !unicode_ident::is_xid_start(first_char) {
        return Err(ParseError::malformed(format!(
            "Invalid placeholder name: {name}"
        )));
    }

    for c in chars {
        if !unicode_ident::is_xid_continue(c) {
            return Err(ParseError::malformed(format!(
                "Invalid placeholder name: {name}"
            )));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Component {
    Literal(String),
    Capture(String),
    Glob, // *
}

impl Component {
    fn is_glob_like(&self) -> bool {
        matches!(self, Component::Glob | Component::Capture(_))
    }

    fn segment(&self) -> Option<&str> {
        match self {
            Component::Capture(s) => Some(s),
            _ => None,
        }
    }
}

/// A single segment of a path pattern, between path separators (or at the begining or end of the pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    GlobStar, // **
    List(Vec<Component>),
}

impl Segment {
    fn captures(&self) -> impl Iterator<Item = &'_ str> {
        (match self {
            Segment::List(components) => Some(components),
            _ => None,
        })
        .into_iter()
        .flat_map(|components| components.iter().flat_map(|c| c.segment()))
    }
}

impl FromStr for Segment {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseError::EmptySegment);
        }

        // GlobStar must be a segment on its own.
        if s == "**" {
            return Ok(Segment::GlobStar);
        }

        let mut components = Vec::new();

        let mut chars = s.chars().peekable();

        let mut literal_buffer = String::new();

        loop {
            let Some(c) = chars.next() else {
                break;
            };

            let new_component = match c {
                '*' => {
                    // '*' must always be followed by a non-'*'
                    if let Some('*') = chars.peek() {
                        return Err(ParseError::malformed(
                            "A glob star may only appear on its own in a segment.",
                        ));
                    }
                    Some(Component::Glob)
                }
                '{' => {
                    // Start of a placeholder. Parse a name.
                    let mut name = String::new();

                    loop {
                        let Some(c) = chars.next() else {
                            return Err(ParseError::malformed("Unterminated placeholder."));
                        };

                        if c == '}' {
                            break;
                        }
                        name.push(c);
                    }

                    validate_placeholder_name(&name)?;

                    Some(Component::Capture(name))
                }
                c if is_valid_literal_char(c) => {
                    literal_buffer.push(c);
                    None
                }
                c => {
                    return Err(ParseError::malformed(format!(
                        "Invalid character in pattern: {c}"
                    )));
                }
            };

            if let Some(component) = new_component {
                if !literal_buffer.is_empty() {
                    components.push(Component::Literal(literal_buffer.clone()));
                    literal_buffer.clear();
                }
                components.push(component);
            }
        }

        if !literal_buffer.is_empty() {
            components.push(Component::Literal(literal_buffer.clone()));
        }

        assert!(!components.is_empty());

        // Check if we have two consecutive glob-like components.
        for (a, b) in components.iter().tuple_windows() {
            if a.is_glob_like() && b.is_glob_like() {
                return Err(ParseError::malformed(
                    "Two consecutive glob-like components are not allowed.",
                ));
            }
        }

        Ok(Segment::List(components))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SinglePattern {
    captures: Vec<String>,
    segments: Vec<Segment>,
}

impl SinglePattern {
    fn build_hir(&self) -> (Node, Vec<String>) {
        let globstar_hir = Node::from_regex("[^/]+(?:/[^/]+)*").expect("Literal parses");

        let glob_hir = Node::from_regex("[^/]+").expect("Literal parses");

        let dirsep_hir = Node::literal("/");

        // Special case: If the pattern is just "**", then it matches everything.
        if self.segments.iter().all(|s| s == &Segment::GlobStar) {
            return (globstar_hir.clone(), Vec::new());
        }

        let mut segments = self.segments.iter().fuse().peekable();

        let mut concat_hirs = Vec::new();

        // Handle prefix globstar.
        if let Some(Segment::GlobStar) = segments.peek() {
            segments.next();
            assert!(!matches!(segments.peek(), Some(&Segment::GlobStar)));
            let prefix_hir = Node::concat([globstar_hir.clone(), dirsep_hir.clone()]).optional();
            concat_hirs.push(prefix_hir);
        }

        let mut captures = Vec::new();

        while let Some(segment) = segments.next() {
            match segment {
                Segment::GlobStar => {
                    // We've handled it separately as part of the previous step. Skip it.
                }
                Segment::List(components) => {
                    let segment_hirs = components.iter().map(|comp| match comp {
                        Component::Literal(literal) => Node::literal(literal.clone()),
                        Component::Glob => glob_hir.clone(),
                        Component::Capture(name) => {
                            captures.push(name.clone());
                            let index = u32::try_from(captures.len()).unwrap();
                            glob_hir.clone().capture(index, name)
                        }
                    });
                    concat_hirs.extend(segment_hirs);

                    let add_globstar = if let Some(Segment::GlobStar) = segments.peek() {
                        segments.next();
                        true
                    } else {
                        false
                    };

                    if add_globstar {
                        concat_hirs.push(
                            Node::concat([dirsep_hir.clone(), globstar_hir.clone()]).optional(),
                        );
                    }

                    if segments.peek().is_some() {
                        concat_hirs.push(dirsep_hir.clone());
                    }
                }
            }
        }

        (Node::concat(concat_hirs), captures)
    }

    pub fn build_matcher(&self) -> PathMatcher {
        let (hir, captures) = self.build_hir();
        let regex = hir.build_matcher().unwrap();
        PathMatcher::new(vec![regex], captures)
    }
}

impl FromStr for SinglePattern {
    type Err = ParseError;

    fn from_str(pattern_str: &str) -> Result<Self, Self::Err> {
        // A quick-and-dirty implementation. If we need to make this more robust, we
        // can follow a more disciplined and regular approach to parsing each component.

        // We reserve "/" to be the directory separator, and it should never appear
        // in a placeholder name. We must ensure this in the rest of the code.

        let segment_strs: Vec<_> = pattern_str.split('/').collect();

        // This must be at least a size of one, and cannot start or end with an
        // empty string.
        if segment_strs.is_empty() {
            return Err(ParseError::EmptyPattern);
        }

        let segments = segment_strs
            .into_iter()
            .map(Segment::from_str)
            .collect::<Result<Vec<_>, _>>()?;

        let captures = segments
            .iter()
            .flat_map(Segment::captures)
            .map(String::from)
            .collect::<Vec<_>>();

        // Ensure that all captures are unique.
        let captures_set = captures.iter().cloned().collect::<BTreeSet<_>>();
        if captures.len() != captures_set.len() {
            return Err(ParseError::malformed(
                "Duplicate capture names are not allowed.",
            ));
        }

        for (a, b) in segments.iter().tuple_windows() {
            if a == &Segment::GlobStar && b == &Segment::GlobStar {
                return Err(ParseError::malformed(
                    "Two consecutive globstar segments are not allowed.",
                ));
            }
        }

        Ok(Self { captures, segments })
    }
}

/// A set of unordered patterns. All patterns must have the same set of placeholders.
#[derive(Debug)]
pub(crate) struct Pattern {
    patterns: Vec<SinglePattern>,
}

impl Pattern {
    pub(crate) fn placeholders(&self) -> impl IntoIterator<Item = &'_ str> {
        self.patterns[0].captures.iter().map(String::as_str)
    }

    pub(crate) fn merge(self, other: Pattern) -> Result<Self, MergeError> {
        if !helpers::iter::eq_unordered(
            self.patterns.iter().map(|p| &p.captures),
            other.patterns.iter().map(|p| &p.captures),
        ) {
            return Err(MergeError::DifferentPlaceholders);
        }
        Ok(Self {
            patterns: [self.patterns, other.patterns].concat(),
        })
    }

    pub(crate) fn build_matcher(&self) -> PathMatcher {
        let mut regexes = Vec::new();
        for pattern in &self.patterns {
            let (hir, _) = pattern.build_hir();
            regexes.push(hir.build_matcher().expect("Ensured by struct invariants"));
        }

        let captures = self.patterns[0].captures.clone();

        PathMatcher::new(regexes, captures)
    }
}

impl FromStr for Pattern {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            patterns: vec![s.parse::<SinglePattern>()?],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_validation() {
        // Valid placeholder
        assert!(matches!(
            "{valid_name}".parse::<Segment>(),
            Ok(Segment::List(_))
        ));

        // Invalid: Starts with number/invalid char
        assert!(matches!(
            "{1invalid}".parse::<Segment>(),
            Err(ParseError::MalformedPattern(_))
        ));

        // Invalid: Empty
        // Note: The logic in `from_str` loops until '}'. If empty "{}", name is "".
        // `validate_placeholder_name` checks `chars.next()`. If None, returns `EmptyPlaceholder`.
        assert!(matches!(
            "{}".parse::<Segment>(),
            Err(ParseError::EmptyPlaceholder)
        ));
    }

    #[test]
    fn test_segment_constraints() {
        // Empty segment (e.g. "//")
        assert!(matches!(
            "".parse::<Segment>(),
            Err(ParseError::EmptySegment)
        ));

        // Double star mixed (e.g. "**a")
        // '**' is special cased. If it's not exact "**", it goes to loop.
        // First char '*' is Glob. Next char is '*'.
        // `if let Some('*') = chars.peek()` -> Error.
        assert!(matches!(
            "**a".parse::<Segment>(),
            Err(ParseError::MalformedPattern(_))
        ));

        // Literal chars
        assert!("valid-name".parse::<Segment>().is_ok());
        // Spaces are invalid
        assert!(matches!(
            "invalid name".parse::<Segment>(),
            Err(ParseError::MalformedPattern(_))
        ));
    }

    #[test]
    fn test_pattern_structure() {
        // "a/b"
        assert!("a/b".parse::<Pattern>().is_ok());

        // Trailing slash "a/" -> splits to "a", "" -> EmptySegment
        // Note: "a/".split('/') yields ["a", ""]. "a" parses OK. "" parses Err(EmptySegment).
        assert!(matches!(
            "a/".parse::<Pattern>(),
            Err(ParseError::EmptySegment)
        ));

        // Leading slash "/a" -> splits to "", "a" -> EmptySegment
        assert!(matches!(
            "/a".parse::<Pattern>(),
            Err(ParseError::EmptySegment)
        ));
    }

    #[test]
    fn test_invalid_literals() {
        // Spec prohibits characters outside L, N, M groups and '-', '_', '.'
        // Common chars like '+', '!', '?' should fail.
        assert!(matches!(
            "c++.rs".parse::<Pattern>(),
            Err(ParseError::MalformedPattern(_))
        ));
        assert!(matches!(
            "foo!bar".parse::<Pattern>(),
            Err(ParseError::MalformedPattern(_))
        ));

        // Backslash is not in the allowed set and escaping is removed from spec.
        assert!(matches!(
            r"foo\bar".parse::<Pattern>(),
            Err(ParseError::MalformedPattern(_))
        ));
    }

    #[test]
    fn test_unmatched_braces() {
        // Spec: "closing brace `}` is not permitted outside of a placeholder"

        assert!(matches!(
            "foo}bar".parse::<Pattern>(),
            Err(ParseError::MalformedPattern(_))
        ));
    }

    #[test]
    fn test_consecutive_globs() {
        // Spec: "there must not be two consecutive glob-likes"

        // Two placeholders
        let res = "{id}{type}".parse::<Pattern>();
        assert!(
            matches!(res, Err(ParseError::MalformedPattern(_))),
            "Consecutive placeholders should fail, got {res:?}"
        );

        // Placeholder and glob
        let res = "{id}*".parse::<Pattern>();
        assert!(
            matches!(res, Err(ParseError::MalformedPattern(_))),
            "Placeholder followed by glob should fail, got {res:?}"
        );

        // Glob and placeholder
        let res = "*{id}".parse::<Pattern>();
        assert!(
            matches!(res, Err(ParseError::MalformedPattern(_))),
            "Glob followed by placeholder should fail, got {res:?}"
        );

        // Triple globs
        let res = "***".parse::<Pattern>();
        assert!(
            matches!(res, Err(ParseError::MalformedPattern(_))),
            "Triple glob should fail, got {res:?}"
        );
    }

    #[test]
    fn test_simple_match() {
        let pattern = "foo".parse::<Pattern>().unwrap();
        let matcher = pattern.build_matcher();
        let res = matcher.match_path("foo").unwrap();
        assert!(res.is_some());
    }

    #[test]
    fn test_simple_match_failed() {
        let pattern = "foo".parse::<Pattern>().unwrap();
        let matcher = pattern.build_matcher();
        let res = matcher.match_path("bar").unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn test_placeholder_match() {
        let pattern = "files/{id}.{ext}".parse::<Pattern>().unwrap();
        let matcher = pattern.build_matcher();
        let res = matcher.match_path("files/123.txt").unwrap();
        assert!(res.is_some());
        let res = res.unwrap();
        assert_eq!(res.normalized_path(), "files/123.txt");
        assert_eq!(res.properties().len(), 2);
        assert_eq!(res.properties()["id"], "123");
        assert_eq!(res.properties()["ext"], "txt");
    }

    #[test]
    fn test_globstar_match() {
        let pattern = "src/**/{name}.rs".parse::<Pattern>().unwrap();
        let matcher = pattern.build_matcher();
        let res = matcher.match_path("src/lib.rs").unwrap();
        assert!(res.is_some());
        let res = res.unwrap();
        assert_eq!(res.normalized_path(), "src/lib.rs");
        assert_eq!(res.properties().len(), 1);
        assert_eq!(res.properties()["name"], "lib");

        let res = matcher.match_path("src/bin/main.rs").unwrap();
        assert!(res.is_some());
        let res = res.unwrap();
        assert_eq!(res.normalized_path(), "src/bin/main.rs");
        assert_eq!(res.properties().len(), 1);
        assert_eq!(res.properties()["name"], "main");
    }

    #[test]
    fn test_valid_ambiguous() {
        let pattern = "**/foo/**".parse::<Pattern>().unwrap();
        let matcher = pattern.build_matcher();
        let res = matcher.match_path("foo").unwrap();
        assert!(res.is_some());

        let res = matcher.match_path("foo/foo").unwrap();
        assert!(res.is_some());
    }

    #[test]
    fn test_invalid_ambiguous_match() {
        let pattern = "**/{id}/**".parse::<Pattern>().unwrap();
        let matcher = pattern.build_matcher();
        let res = matcher.match_path("foo").unwrap();
        assert!(res.is_some());

        let res = matcher.match_path("foo/bar");
        assert!(res.is_err(), "Should be ambiguous: {res:?}");
    }

    #[test]
    fn test_ambiguity_based_on_posititon() {
        let pattern = "**/{id}/**".parse::<Pattern>().unwrap();
        let matcher = pattern.build_matcher();

        let res = matcher.match_path("foo/foo");
        assert!(res.is_err(), "Should be ambiguous: {res:?}");
    }
}
