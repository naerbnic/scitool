use crate::utils::compression::dcl::DictType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BackrefMatch {
    pub offset: usize,
    pub length: usize,
}

pub(super) struct Dictionary {
    data: Vec<u8>,
    pos: usize,
    mask: usize,
    max_offset: usize,
}

impl Dictionary {
    pub(crate) fn new(dict_type: DictType) -> Self {
        let mask = dict_type.dict_size() - 1;
        Self {
            data: vec![0u8; dict_type.dict_size()],
            pos: 0,
            mask,
            max_offset: 0,
        }
    }

    fn match_len(&self, back_offset: usize, data: &[u8]) -> usize {
        assert!(back_offset > 0);
        assert!(back_offset <= self.data.len());
        let mut byte_pairs = self.self_overlap_cursor(back_offset).zip(data);
        byte_pairs.position(|(a, b)| a != *b).unwrap_or(data.len())
    }

    pub(super) fn find_best_match(&self, data: &[u8]) -> Option<BackrefMatch> {
        let mut curr_match: Option<BackrefMatch> = None;

        for back_offset in 1..=self.max_offset {
            let length = self.match_len(back_offset, data);
            if length > curr_match.map_or(0, |m| m.length) {
                curr_match = Some(BackrefMatch {
                    offset: back_offset,
                    length,
                });
            }
        }
        curr_match
    }

    pub(super) fn append_data(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            let copy_len = std::cmp::min(self.data.len() - self.pos, data.len());
            self.data[self.pos..][..copy_len].copy_from_slice(data);
            self.pos = (self.pos + copy_len) & self.mask;
            self.max_offset = (self.max_offset + copy_len).min(self.data.len());
            data = &data[copy_len..];
        }
    }

    fn self_overlap_cursor(&self, back_offset: usize) -> DictionaryCursor<'_> {
        assert!(back_offset > 0);
        let cursor_pos = self.pos.wrapping_sub(back_offset) & self.mask;
        DictionaryCursor {
            dict: self,
            start: cursor_pos,
            pos: cursor_pos,
        }
    }
}

struct DictionaryCursor<'a> {
    dict: &'a Dictionary,
    start: usize,
    pos: usize,
}

impl Iterator for DictionaryCursor<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.dict.data[self.pos];
        self.pos = (self.pos + 1) & self.dict.mask;
        if self.pos == self.dict.pos {
            // We implement this to allow for overlapping copies. When we reach the
            // end of the dictionary, we wrap around back to where the cursor started.
            // This should emulate the cursor's data being copied into the output buffer.
            self.pos = self.start;
        }
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictionary() {
        let mut dict = Dictionary::new(DictType::Size1024);
        let data = b"Hello, world!";
        dict.append_data(data);
        assert_eq!(
            dict.find_best_match(b"Hello"),
            Some(BackrefMatch {
                offset: 13,
                length: 5,
            })
        );

        assert_eq!(
            dict.find_best_match(b"world!"),
            Some(BackrefMatch {
                offset: 6,
                length: 6,
            })
        );
    }

    #[test]
    fn test_dictionary_wrap() {
        let mut dict = Dictionary::new(DictType::Size1024);
        let data = b"A";
        dict.append_data(data);
        assert_eq!(
            dict.find_best_match(b"AAAAA"),
            Some(BackrefMatch {
                offset: 1,
                length: 5,
            })
        );
    }

    #[test]
    fn finds_closest_match() {
        let mut dict = Dictionary::new(DictType::Size1024);
        let data = b"ABABABAB";
        dict.append_data(data);
        assert_eq!(
            dict.find_best_match(b"AB"),
            Some(BackrefMatch {
                offset: 2,
                length: 2,
            })
        );

        assert_eq!(
            dict.find_best_match(b"BA"),
            Some(BackrefMatch {
                offset: 3,
                length: 2,
            })
        );
    }

    #[test]
    fn finds_longest_match() {
        let mut dict = Dictionary::new(DictType::Size1024);
        let data = b"ABABADABC";
        dict.append_data(data);
        assert_eq!(
            dict.find_best_match(b"ABAB"),
            Some(BackrefMatch {
                offset: 9,
                length: 4,
            })
        );

        assert_eq!(
            dict.find_best_match(b"ABA"),
            Some(BackrefMatch {
                offset: 7,
                length: 3,
            })
        );
    }

    #[test]
    fn empty_match_returns_none() {
        let dict = Dictionary::new(DictType::Size1024);
        assert_eq!(dict.find_best_match(b"ABC"), None);
    }

    #[test]
    fn no_match_returns_none() {
        let mut dict = Dictionary::new(DictType::Size1024);
        let data = b"ABCDEFGH";
        dict.append_data(data);
        assert_eq!(dict.find_best_match(b"XYZ"), None);
    }

    #[test]
    fn wrapped_data_is_considered() {
        let mut dict = Dictionary::new(DictType::Size1024);
        dict.append_data(b"1234");
        dict.append_data(&b"A".iter().copied().cycle().take(1016).collect::<Vec<u8>>());
        dict.append_data(b"5678");

        // Should still be able to find "ABCD" at offset 1024 (i.e. wrapping around to the start of the dictionary)
        assert_eq!(
            dict.find_best_match(b"1234"),
            Some(BackrefMatch {
                offset: 1024,
                length: 4,
            })
        );

        // Writing one more byte should push out the "1" at the start of the dictionary
        dict.append_data(b"Z");

        assert_eq!(dict.find_best_match(b"1234"), None);

        // But we should still be able to find "234" at offset 1023
        assert_eq!(
            dict.find_best_match(b"234"),
            Some(BackrefMatch {
                offset: 1024,
                length: 3,
            })
        );

        // We can also find "5678" at offset 5, even though it's literal position
        // is in front of the current position.
        assert_eq!(
            dict.find_best_match(b"5678"),
            Some(BackrefMatch {
                offset: 5,
                length: 4,
            })
        );
    }

    
}
