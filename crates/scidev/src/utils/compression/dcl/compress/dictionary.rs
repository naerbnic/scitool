use super::{DictType, index_cache::IndexCache};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BackrefMatch {
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MatchLengthParams {
    pub max: usize,
    pub min: usize,
    pub sufficient: Option<usize>,
}

pub(super) struct Dictionary {
    data: Vec<u8>,
    pos: usize,
    mask: usize,
    max_offset: usize,
    cache: IndexCache,
}

impl Dictionary {
    pub(crate) fn new(dict_type: DictType) -> Self {
        let mask = dict_type.dict_size() - 1;
        Self {
            data: vec![0u8; dict_type.dict_size()],
            pos: 0,
            mask,
            max_offset: 0,
            cache: IndexCache::new(),
        }
    }

    fn match_len(&self, max_length: usize, back_offset: usize, data: &[u8]) -> usize {
        assert!(back_offset > 0);
        assert!(back_offset <= self.data.len());
        let mut byte_pairs = self
            .self_overlap_cursor(back_offset)
            .zip(data)
            .take(max_length);
        byte_pairs.position(|(a, b)| a != *b).unwrap_or(data.len())
    }

    pub(super) fn find_best_match(
        &self,
        params: &MatchLengthParams,
        data: &[u8],
    ) -> Option<BackrefMatch> {
        if data.is_empty() {
            return None;
        }
        let mut curr_match: Option<BackrefMatch> = None;

        // dbg!(data[0], self.cache.get_entries(data[0]).collect::<Vec<_>>());

        for index in self.cache.get_entries(data[0]) {
            if self.data[index] != data[0] {
                break;
            }
            let back_offset = if self.pos == index {
                self.data.len()
            } else {
                self.pos.wrapping_sub(index) & self.mask
            };
            let length = self.match_len(params.max, back_offset, data);
            if length < params.min {
                continue;
            }
            if let Some(sufficient) = params.sufficient
                && length >= sufficient
            {
                return Some(BackrefMatch {
                    offset: back_offset,
                    length,
                });
            }
            assert!(length <= params.max);
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
            let (curr_data, next_data) = data.split_at(copy_len);
            self.data[self.pos..][..copy_len].copy_from_slice(curr_data);
            for (i, &b) in curr_data.iter().enumerate() {
                self.cache.insert(b, (self.pos + i) & self.mask);
            }
            self.pos = (self.pos + copy_len) & self.mask;
            self.max_offset = (self.max_offset + copy_len).min(self.data.len());
            data = next_data;
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
    use crate::utils::compression::dcl::compress::MAX_BACKREF_LENGTH;

    use super::*;

    const TEST_PARAMS: MatchLengthParams = MatchLengthParams {
        max: MAX_BACKREF_LENGTH,
        min: 2,
        sufficient: None,
    };

    #[test]
    fn test_dictionary() {
        let mut dict = Dictionary::new(DictType::Size1024);
        let data = b"Hello, world!";
        dict.append_data(data);
        assert_eq!(
            dict.find_best_match(&TEST_PARAMS, b"Hello"),
            Some(BackrefMatch {
                offset: 13,
                length: 5,
            })
        );

        assert_eq!(
            dict.find_best_match(&TEST_PARAMS, b"world!"),
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
            dict.find_best_match(&TEST_PARAMS, b"AAAAA"),
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
            dict.find_best_match(&TEST_PARAMS, b"AB"),
            Some(BackrefMatch {
                offset: 2,
                length: 2,
            })
        );

        assert_eq!(
            dict.find_best_match(&TEST_PARAMS, b"BA"),
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
            dict.find_best_match(&TEST_PARAMS, b"ABAB"),
            Some(BackrefMatch {
                offset: 9,
                length: 4,
            })
        );

        assert_eq!(
            dict.find_best_match(&TEST_PARAMS, b"ABA"),
            Some(BackrefMatch {
                offset: 7,
                length: 3,
            })
        );
    }

    #[test]
    fn empty_match_returns_none() {
        let dict = Dictionary::new(DictType::Size1024);
        assert_eq!(dict.find_best_match(&TEST_PARAMS, b"ABC"), None);
    }

    #[test]
    fn no_match_returns_none() {
        let mut dict = Dictionary::new(DictType::Size1024);
        let data = b"ABCDEFGH";
        dict.append_data(data);
        assert_eq!(dict.find_best_match(&TEST_PARAMS, b"XYZ"), None);
    }

    #[test]
    fn wrapped_data_is_considered() {
        let mut dict = Dictionary::new(DictType::Size1024);
        dict.append_data(b"1234");
        dict.append_data(&b"A".iter().copied().cycle().take(1016).collect::<Vec<u8>>());
        dict.append_data(b"5678");

        // Should still be able to find "ABCD" at offset 1024 (i.e. wrapping around to the start of the dictionary)
        assert_eq!(
            dict.find_best_match(&TEST_PARAMS, b"1234"),
            Some(BackrefMatch {
                offset: 1024,
                length: 4,
            })
        );

        // Writing one more byte should push out the "1" at the start of the dictionary
        dict.append_data(b"Z");

        assert_eq!(dict.find_best_match(&TEST_PARAMS, b"1234"), None);

        // But we should still be able to find "234" at offset 1023
        assert_eq!(
            dict.find_best_match(&TEST_PARAMS, b"234"),
            Some(BackrefMatch {
                offset: 1024,
                length: 3,
            })
        );

        // We can also find "5678" at offset 5, even though it's literal position
        // is in front of the current position.
        assert_eq!(
            dict.find_best_match(&TEST_PARAMS, b"5678"),
            Some(BackrefMatch {
                offset: 5,
                length: 4,
            })
        );
    }
}
