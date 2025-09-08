use crate::utils::compression::errors::UnexpectedEndOfInput;
use bitter::BitReader;

enum HuffmanTableEntry<T> {
    Leaf(T),
    Branch(usize, usize),
}

pub(super) struct HuffmanTable<T> {
    entries: Vec<HuffmanTableEntry<T>>,
}

impl<T> HuffmanTable<T> {
    pub(super) fn builder() -> Builder<T> {
        Builder {
            entries: Vec::new(),
        }
    }

    pub(super) fn lookup<R: BitReader>(&self, reader: &mut R) -> Result<&T, UnexpectedEndOfInput> {
        let mut pos = 0;
        loop {
            match &self.entries[pos] {
                HuffmanTableEntry::Leaf(value) => return Ok(value),
                HuffmanTableEntry::Branch(left, right) => {
                    let bit = reader.read_bit().ok_or(UnexpectedEndOfInput)?;
                    pos = if bit { *right } else { *left };
                }
            }
        }
    }
}

pub(super) struct Builder<T> {
    entries: Vec<HuffmanTableEntry<T>>,
}

impl<T> Builder<T> {
    pub(super) fn add_branch(&mut self, pos: usize, left: usize, right: usize) -> &mut Self {
        assert_eq!(pos, self.entries.len());
        self.entries.push(HuffmanTableEntry::Branch(left, right));
        self
    }

    pub(super) fn add_leaf(&mut self, pos: usize, value: T) -> &mut Self {
        assert_eq!(pos, self.entries.len());
        self.entries.push(HuffmanTableEntry::Leaf(value));
        self
    }

    pub(super) fn build(&mut self) -> HuffmanTable<T> {
        HuffmanTable {
            entries: std::mem::take(&mut self.entries),
        }
    }
}
