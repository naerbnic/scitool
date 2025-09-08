use crate::utils::compression::{errors::UnexpectedEndOfInput, writer::BitWriter};
use bitter::BitReader;

enum HuffmanTableEntry {
    Leaf(usize),
    Branch(usize, usize),
}

pub(super) struct HuffmanTable<T> {
    alphabet: Vec<T>,
    entries: Vec<HuffmanTableEntry>,
}

impl<T> HuffmanTable<T> {
    pub(super) fn builder() -> Builder<T> {
        Builder {
            alphabet: Vec::new(),
            entries: Vec::new(),
        }
    }

    pub(super) fn lookup<R: BitReader>(&self, reader: &mut R) -> Result<&T, UnexpectedEndOfInput> {
        let mut pos = 0;
        loop {
            match &self.entries[pos] {
                HuffmanTableEntry::Leaf(index) => return Ok(&self.alphabet[*index]),
                HuffmanTableEntry::Branch(left, right) => {
                    let bit = reader.read_bit().ok_or(UnexpectedEndOfInput)?;
                    pos = if bit { *right } else { *left };
                }
            }
        }
    }
}

pub(super) struct Builder<T> {
    alphabet: Vec<T>,
    entries: Vec<HuffmanTableEntry>,
}

impl<T> Builder<T>
where
    T: Ord,
{
    pub(super) fn add_branch(&mut self, pos: usize, left: usize, right: usize) -> &mut Self {
        assert_eq!(pos, self.entries.len());
        self.entries.push(HuffmanTableEntry::Branch(left, right));
        self
    }

    pub(super) fn add_leaf(&mut self, pos: usize, value: T) -> &mut Self {
        assert_eq!(pos, self.entries.len());
        let index = self.alphabet.len();
        self.alphabet.push(value);
        self.entries.push(HuffmanTableEntry::Leaf(index));
        self
    }

    pub(super) fn build(&mut self) -> HuffmanTable<T> {
        let mut alphabet_sort_vec: Vec<_> = std::mem::take(&mut self.alphabet)
            .into_iter()
            .enumerate()
            .collect();
        alphabet_sort_vec.sort_by(|p1, p2| p1.1.cmp(&p2.1));
        let mut old_to_new_indexes = vec![0usize; alphabet_sort_vec.len()];
        for (new_index, (old_index, _)) in alphabet_sort_vec.iter().enumerate() {
            old_to_new_indexes[*old_index] = new_index;
        }
        let new_alphabet: Vec<T> = alphabet_sort_vec.into_iter().map(|(_, v)| v).collect();
        HuffmanTable {
            alphabet: new_alphabet,
            entries: std::mem::take(&mut self.entries)
                .into_iter()
                .map(|entry| match entry {
                    HuffmanTableEntry::Leaf(old_index) => {
                        HuffmanTableEntry::Leaf(old_to_new_indexes[old_index])
                    }
                    HuffmanTableEntry::Branch(left, right) => {
                        HuffmanTableEntry::Branch(left, right)
                    }
                })
                .collect(),
        }
    }
}
