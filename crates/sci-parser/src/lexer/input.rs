use std::{ops::Range, rc::Rc};

use nom::InputLength;

mod location;

pub(super) use location::{InputOffset, InputRange};

#[derive(Debug)]
pub(super) struct InputContents<'a> {
    contents: &'a str,
    /// Byte offsets of the ends of strings in the contents.
    line_end_offsets: Vec<usize>,
}

impl<'a> InputContents<'a> {
    pub fn new(contents: &'a str) -> Self {
        let mut line_end_offsets = vec![0];
        for (i, c) in contents.char_indices() {
            let post_char_offset = i + c.len_utf8();
            if c == '\n' {
                line_end_offsets.push(post_char_offset);
            } else if c == '\r' {
                // See if the character after this is a newline.
                if let Some('\n') = contents[post_char_offset..].chars().next() {
                    line_end_offsets.push(post_char_offset + 1);
                } else {
                    line_end_offsets.push(post_char_offset);
                }
            }
        }
        Self {
            contents,
            line_end_offsets,
        }
    }

    pub fn get_text_offset(&self, absolute_offset: usize) -> InputOffset {
        let line_index = match self.line_end_offsets.binary_search(&absolute_offset) {
            Ok(i) => i + 1,
            Err(i) => i,
        };

        let line_start_offset = match line_index {
            0 => 0,
            _ => self.line_end_offsets[line_index - 1],
        };
        let line_prefix = &self.contents[line_start_offset..absolute_offset];
        let num_chars = line_prefix.chars().count();
        InputOffset {
            offset: absolute_offset,
            line_index,
            line_char_offset: num_chars,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Input<'a> {
    contents: Rc<InputContents<'a>>,
    // The range within the contents that this input represents.
    range: Range<usize>,
}

impl<'a> Input<'a> {
    pub fn new(contents: &'a str) -> Self {
        let contents = Rc::new(InputContents::new(contents.as_ref()));
        let range_end = contents.contents.len();
        Self {
            contents,
            range: 0..range_end,
        }
    }
    pub fn input_offset(&self) -> InputOffset {
        self.contents.get_text_offset(self.range.start)
    }
    fn content_slice(&self) -> &'a str {
        &self.contents.contents[self.range.clone()]
    }
}

impl InputLength for Input<'_> {
    fn input_len(&self) -> usize {
        self.contents.contents.len()
    }
}

impl<'a> nom::InputIter for Input<'a> {
    type Item = char;
    type Iter = std::str::CharIndices<'a>;
    type IterElem = std::str::Chars<'a>;

    fn iter_indices(&self) -> Self::Iter {
        self.content_slice().char_indices()
    }

    fn iter_elements(&self) -> Self::IterElem {
        self.content_slice().chars()
    }

    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.content_slice().find(predicate)
    }

    fn slice_index(&self, count: usize) -> Result<usize, nom::Needed> {
        if self.input_len() >= count {
            Ok(count)
        } else {
            Err(nom::Needed::new(count - self.input_len()))
        }
    }
}

impl nom::InputTake for Input<'_> {
    fn take(&self, count: usize) -> Self {
        let end_position = self.range.start + count;
        assert!(self.contents.contents.is_char_boundary(end_position));
        let range = self.range.start..end_position;
        Input {
            range,
            ..self.clone()
        }
    }

    fn take_split(&self, count: usize) -> (Self, Self) {
        let split_position = self.range.start + count;
        assert!(self.contents.contents.is_char_boundary(split_position));
        let start_range = self.range.start..split_position;
        let end_range = split_position..self.range.end;
        (
            Input {
                range: end_range,
                ..self.clone()
            },
            Input {
                range: start_range,
                ..self.clone()
            },
        )
    }
}

impl nom::UnspecializedInput for Input<'_> {}

#[cfg(test)]
mod tests {
    use nom::{InputIter, InputTake};

    use super::*;

    #[test]
    fn all_chars_are_listed() {
        let contents = Input::new("abcあ");
        let chars = contents.iter_elements().collect::<Vec<_>>();
        assert_eq!(chars, vec!['a', 'b', 'c', 'あ']);
    }

    #[test]
    fn all_chars_and_indices_are_listed() {
        let contents = Input::new("abcあdef");
        let chars = contents.iter_indices().collect::<Vec<_>>();
        assert_eq!(
            chars,
            vec![
                (0, 'a'),
                (1, 'b'),
                (2, 'c'),
                (3, 'あ'),
                (6, 'd'),
                (7, 'e'),
                (8, 'f')
            ]
        );
    }

    #[test]
    fn position_of_char_is_correct() {
        let contents = Input::new("abcあdef");
        assert_eq!(contents.position(|c| c == 'あ'), Some(3));
        assert_eq!(contents.position(|c| c == 'e'), Some(7));
        assert_eq!(contents.position(|c| c == 'z'), None);
    }

    #[test]
    fn take_obtains_prefix() {
        let contents = Input::new("abcあdef");
        let prefix = contents.take(6);
        assert_eq!(prefix.content_slice(), "abcあ");
    }

    #[test]
    #[should_panic]
    fn take_panics_on_non_char_boundary() {
        let contents = Input::new("abcあdef");
        let _ = contents.take(5);
    }

    #[test]
    fn take_split_works() {
        let contents = Input::new("abcあdef");
        let (suffix, prefix) = contents.take_split(6);
        assert_eq!(prefix.content_slice(), "abcあ");
        assert_eq!(suffix.content_slice(), "def");
    }
}
