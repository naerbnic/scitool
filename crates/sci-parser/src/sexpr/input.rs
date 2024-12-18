use std::ops::Range;

use nom::{InputIter, InputLength, InputTake, Slice};

pub struct Input<'a, T> {
    contents: &'a [T],
    range: Range<usize>,
}

impl<T> std::fmt::Debug for Input<'_, T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.content_slice())
    }
}

impl<'a, T> Input<'a, T> {
    pub fn new(contents: &'a [T]) -> Self {
        Self {
            contents,
            range: 0..contents.len(),
        }
    }

    pub fn input_offset(&self) -> usize {
        self.range.start
    }

    pub fn content_slice(&self) -> &'a [T] {
        &self.contents[self.range.clone()]
    }
}

impl<T> InputLength for Input<'_, T> {
    fn input_len(&self) -> usize {
        self.range.len()
    }
}

impl<'a, T> InputIter for Input<'a, T> {
    type Item = &'a T;
    type Iter = std::iter::Enumerate<std::slice::Iter<'a, T>>;
    type IterElem = std::slice::Iter<'a, T>;

    fn iter_indices(&self) -> Self::Iter {
        self.content_slice().iter().enumerate()
    }

    fn iter_elements(&self) -> Self::IterElem {
        self.content_slice().iter()
    }

    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.content_slice().iter().position(predicate)
    }

    fn slice_index(&self, count: usize) -> Result<usize, nom::Needed> {
        if self.input_len() >= count {
            Ok(count)
        } else {
            Err(nom::Needed::new(count - self.input_len()))
        }
    }
}

impl<T> InputTake for Input<'_, T> {
    fn take(&self, count: usize) -> Self {
        assert!(self.range.start + count <= self.range.end);
        Input {
            contents: self.contents,
            range: self.range.start..self.range.start + count,
        }
    }

    fn take_split(&self, count: usize) -> (Self, Self) {
        let split_position = self.range.start + count;
        assert!(split_position <= self.range.end);
        let start_range = self.range.start..split_position;
        let end_range = split_position..self.range.end;
        (
            Input {
                contents: self.contents,
                range: end_range,
            },
            Input {
                contents: self.contents,
                range: start_range,
            },
        )
    }
}

impl<T, R> Slice<R> for Input<'_, T>
where
    R: std::ops::RangeBounds<usize>,
{
    fn slice(&self, range: R) -> Self {
        let start_offset = match range.start_bound() {
            std::ops::Bound::Included(&start) => start,
            std::ops::Bound::Excluded(&start) => start + 1,
            std::ops::Bound::Unbounded => 0,
        };
        let end_offset = match range.end_bound() {
            std::ops::Bound::Included(&end) => end + 1,
            std::ops::Bound::Excluded(&end) => end,
            std::ops::Bound::Unbounded => self.range.end - self.range.start,
        };
        let new_range_start = self.range.start + start_offset;
        let new_range_end = self.range.start + end_offset;
        assert!(new_range_start <= self.range.end);
        assert!(new_range_end <= self.range.end);
        Input {
            contents: self.contents,
            range: new_range_start..new_range_end,
        }
    }
}

impl<T> nom::UnspecializedInput for Input<'_, T> {}
