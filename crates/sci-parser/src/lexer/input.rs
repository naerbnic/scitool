use std::ops::Range;

use nom::InputLength;

use location::TextOffset;

pub(super) mod location;

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

    pub fn get_text_offset(&self, absolute_offset: usize) -> TextOffset {
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
        TextOffset {
            offset: absolute_offset,
            line_index,
            line_char_offset: num_chars,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Input<'a> {
    contents: &'a InputContents<'a>,
    // The range within the contents that this input represents.
    range: Range<usize>,
}

impl<'a> Input<'a> {
    pub fn input_offset(&self) -> TextOffset {
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
        let range = self.range.start..self.range.start + count;
        Input { range, ..*self }
    }

    fn take_split(&self, count: usize) -> (Self, Self) {
        let start_range = self.range.start..self.range.start + count;
        let end_range = self.range.start + count..self.range.end;
        (
            Input {
                range: start_range,
                ..*self
            },
            Input {
                range: end_range,
                ..*self
            },
        )
    }
}

impl nom::UnspecializedInput for Input<'_> {}
