#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InputRange {
    start: InputOffset,
    end: InputOffset,
}

impl InputRange {
    pub fn new_empty() -> Self {
        Self {
            start: InputOffset::zero(),
            end: InputOffset::zero(),
        }
    }
    pub fn new(start: InputOffset, end: InputOffset) -> Self {
        Self { start, end }
    }

    // Merges the two ranges to the smallest range that contains both.
    pub fn merge(self, other: Self) -> Self {
        let start = std::cmp::min(self.start, other.start);
        let end = std::cmp::max(self.end, other.end);
        Self { start, end }
    }

    pub fn start(&self) -> InputOffset {
        self.start
    }

    pub fn end(&self) -> InputOffset {
        self.end
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InputOffset {
    /// The absolute offset of this location in the input.
    pub(super) offset: usize,
    /// The absolute offset of the beginning of the line in the input.
    pub(super) line_start_offset: usize,
    pub(super) line_index: usize,
    pub(super) column_index: usize,
}

impl InputOffset {
    pub fn zero() -> Self {
        InputOffset {
            offset: 0,
            line_start_offset: 0,
            line_index: 0,
            column_index: 0,
        }
    }

    pub fn line_index(&self) -> usize {
        self.line_index
    }

    pub fn line_start_offset(&self) -> usize {
        self.line_start_offset
    }

    pub fn line_char_offset(&self) -> usize {
        self.column_index
    }
}

impl std::cmp::Eq for InputOffset {}

impl std::cmp::PartialEq for InputOffset {
    fn eq(&self, other: &Self) -> bool {
        // All comparisons are based only on the offset field
        self.offset == other.offset
    }
}

impl std::cmp::Ord for InputOffset {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.offset.cmp(&other.offset)
    }
}

impl std::cmp::PartialOrd for InputOffset {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::hash::Hash for InputOffset {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.offset.hash(state);
    }
}
