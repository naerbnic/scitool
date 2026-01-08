use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum ParseError {
    #[error("Unterminated placeholder")]
    UnterminatedPlaceholder,

    #[error("Invalid escape sequence")]
    InvalidEscape(Option<char>),

    #[error("Invalid placeholder: {0:?}")]
    InvalidPlaceholder(String),

    #[error("Invalid character: {0}")]
    InvalidCharacter(char),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ApplyError {
    #[error("Missing property: {0}")]
    MissingProperty(String),
}

fn validate_placeholder_name(str: &str) -> Result<(), ParseError> {
    let mut chars = str.chars();
    let Some(first) = chars.next() else {
        return Err(ParseError::InvalidPlaceholder(str.to_string()));
    };

    if !unicode_ident::is_xid_start(first) {
        return Err(ParseError::InvalidPlaceholder(str.to_string()));
    }

    for c in chars {
        if !unicode_ident::is_xid_continue(c) {
            return Err(ParseError::InvalidPlaceholder(str.to_string()));
        }
    }

    Ok(())
}

#[derive(Debug)]
enum Component {
    Literal(String),
    Placeholder(String),
}

#[derive(Debug)]
pub(crate) struct PropTemplate {
    components: Vec<Component>,
    placeholder_names: BTreeSet<String>,
}

impl PropTemplate {
    pub(crate) fn placeholders(&self) -> impl Iterator<Item = &String> {
        self.placeholder_names.iter()
    }

    pub(crate) fn apply(
        &self,
        properties: &BTreeMap<String, String>,
    ) -> Result<String, ApplyError> {
        let mut result = String::new();
        for component in &self.components {
            match component {
                Component::Literal(s) => result.push_str(s),
                Component::Placeholder(name) => {
                    let Some(value) = properties.get(name) else {
                        return Err(ApplyError::MissingProperty(name.clone()));
                    };
                    result.push_str(value);
                }
            }
        }
        Ok(result)
    }
}

impl FromStr for PropTemplate {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut literal_buffer = String::new();

        let mut placeholder_names = BTreeSet::new();
        let mut components = Vec::new();

        let mut chars = s.chars();

        let mut at_end = false;
        while !at_end {
            let c_opt = chars.next();
            at_end = c_opt.is_none();

            let next_component = if let Some(c) = c_opt {
                match c {
                    '{' => {
                        let mut placeholder = String::new();
                        loop {
                            let Some(c) = chars.next() else {
                                return Err(ParseError::UnterminatedPlaceholder);
                            };

                            if c == '}' {
                                break;
                            }
                            placeholder.push(c);
                        }
                        validate_placeholder_name(&placeholder)?;
                        placeholder_names.insert(placeholder.clone());
                        Some(Component::Placeholder(placeholder))
                    }
                    '\\' => {
                        let Some(esc_c) = chars.next() else {
                            return Err(ParseError::InvalidEscape(None));
                        };

                        let real_c = match esc_c {
                            '{' | '}' | '\\' => esc_c,
                            _ => return Err(ParseError::InvalidEscape(Some(esc_c))),
                        };

                        literal_buffer.push(real_c);
                        None
                    }
                    '}' => {
                        return Err(ParseError::InvalidCharacter(c));
                    }
                    _ => {
                        literal_buffer.push(c);
                        None
                    }
                }
            } else {
                None
            };

            let has_new_component = next_component.is_some();

            if (has_new_component || at_end) && !literal_buffer.is_empty() {
                components.push(Component::Literal(literal_buffer));
                literal_buffer = String::new();
            }

            if let Some(component) = next_component {
                components.push(component);
            }

            if at_end {
                break;
            }
        }

        if !literal_buffer.is_empty() {
            components.push(Component::Literal(literal_buffer));
        }

        Ok(Self {
            components,
            placeholder_names,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::test::{assert_matches, make_map};

    #[test]
    fn test_literal() {
        let template = PropTemplate::from_str("abc").unwrap();
        assert_eq!(template.apply(&BTreeMap::new()).unwrap(), "abc");
    }

    #[test]
    fn test_placeholder() {
        let template = PropTemplate::from_str("{foo}").unwrap();
        assert_eq!(template.apply(&make_map([("foo", "bar")])).unwrap(), "bar");
    }

    #[test]
    fn test_multi_placeholder() {
        let template = PropTemplate::from_str("{foo}{bar}").unwrap();
        assert_eq!(
            template
                .apply(&make_map([("foo", "bar"), ("bar", "baz")]))
                .unwrap(),
            "barbaz"
        );
    }

    #[test]
    fn test_repeated_placeholder() {
        let template = PropTemplate::from_str("{foo}:{foo}").unwrap();
        // Should only have one placeholder, even with two instances.
        assert_eq!(template.placeholders().collect::<Vec<_>>(), ["foo"]);
        // Expands in both locations
        assert_eq!(
            template.apply(&make_map([("foo", "bar")])).unwrap(),
            "bar:bar"
        );
    }

    #[test]
    fn test_interleaved() {
        let template = PropTemplate::from_str("<<{foo}abc{bar}>>").unwrap();
        assert_eq!(
            template
                .apply(&make_map([("foo", "bar"), ("bar", "baz")]))
                .unwrap(),
            "<<barabcbaz>>"
        );
    }

    #[test]
    fn test_escaped() {
        let template = PropTemplate::from_str("\\{foo\\}").unwrap();
        assert_eq!(
            template.apply(&make_map([("foo", "bar")])).unwrap(),
            "{foo}"
        );
    }

    #[test]
    fn test_invalid_closing_brace() {
        assert_matches!(
            PropTemplate::from_str("foo}"),
            Err(ParseError::InvalidCharacter('}'))
        );
    }

    #[test]
    fn test_unterminated_placeholder() {
        assert_matches!(
            PropTemplate::from_str("{foo"),
            Err(ParseError::UnterminatedPlaceholder)
        );
    }

    #[test]
    fn test_invalid_escape() {
        assert_matches!(
            PropTemplate::from_str("\\n"),
            Err(ParseError::InvalidEscape(Some('n')))
        );
    }

    #[test]
    fn test_terminating_escaped() {
        assert_matches!(
            PropTemplate::from_str("\\"),
            Err(ParseError::InvalidEscape(None))
        );
    }

    #[test]
    fn valid_placeholder_names() {
        fn assert_valid_name(s: &str) {
            assert!(PropTemplate::from_str(s).is_ok(), "Failed to parse: {s}");
        }
        // Plain name
        assert_valid_name("{foo}");
        // Name with underscores
        assert_valid_name("{foo_bar}");
        // Name with non-initial numbers
        assert_valid_name("{f123}");
        // Name with non-ASCII unicode, but valid identifier
        assert_valid_name("{あいうえお_かきくけこ}");
    }

    #[test]
    fn test_invalid_placeholder_names() {
        fn assert_invalid_name(s: &str) {
            assert_matches!(
                PropTemplate::from_str(s),
                Err(ParseError::InvalidPlaceholder(_))
            );
        }
        // Bad initial character
        assert_invalid_name("{123}");
        // Spaces not allowed
        assert_invalid_name("{a b c}");
        // Nonstandard punctuation not allowed
        assert_invalid_name("{a—b}");
        assert_invalid_name("{#keyword}");
    }
}
