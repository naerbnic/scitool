mod matcher;
mod pattern;
mod regex;

pub(crate) use matcher::{MatchError, PathMatcher};
pub(crate) use pattern::{MergeError, ParseError, Pattern};
