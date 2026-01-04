use itertools::Itertools;
use std::{borrow::Cow, str::FromStr};
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

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
}

/// A single segment of a path pattern, between path separators (or at the begining or end of the pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    GlobStar, // **
    List(Vec<Component>),
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
pub struct Pattern {
    segments: Vec<Segment>,
}

impl FromStr for Pattern {
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

        Ok(Self {
            segments: segment_strs
                .into_iter()
                .map(Segment::from_str)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_literal() {
        let p: Pattern = "src/main.rs".parse().unwrap();
        assert_eq!(p.segments.len(), 2);
        assert_eq!(
            p.segments[0],
            Segment::List(vec![Component::Literal("src".into())])
        );
        assert_eq!(
            p.segments[1],
            Segment::List(vec![Component::Literal("main.rs".into())])
        );
    }

    #[test]
    fn test_placeholders() {
        let p: Pattern = "files/{id}.{ext}".parse().unwrap();
        assert_eq!(p.segments.len(), 2);
        assert_eq!(
            p.segments[1],
            Segment::List(vec![
                Component::Capture("id".into()),
                Component::Literal(".".into()),
                Component::Capture("ext".into())
            ])
        );
    }

    #[test]
    fn test_glob() {
        let p: Pattern = "*.rs".parse().unwrap();
        assert_eq!(
            p.segments[0],
            Segment::List(vec![Component::Glob, Component::Literal(".rs".into())])
        );
    }

    #[test]
    fn test_globstar() {
        let p: Pattern = "src/**/{name}.rs".parse().unwrap();
        assert_eq!(p.segments.len(), 3);
        assert_eq!(p.segments[1], Segment::GlobStar);
    }

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
        let p = "a/b".parse::<Pattern>().unwrap();
        assert_eq!(p.segments.len(), 2);

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
}
