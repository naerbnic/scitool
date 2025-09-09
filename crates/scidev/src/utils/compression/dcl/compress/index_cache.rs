use std::collections::VecDeque;

pub(crate) struct IndexCache {
    buckets: Vec<VecDeque<u16>>,
}

impl IndexCache {
    pub(crate) fn new() -> Self {
        let mut buckets = Vec::with_capacity(256);
        for _ in 0..=255 {
            buckets.push(VecDeque::new());
        }
        Self { buckets }
    }

    pub(crate) fn insert(&mut self, byte: u8, index: usize) {
        let bucket = &mut self.buckets[usize::from(byte)];
        bucket.push_back(u16::try_from(index).expect("Table sizes should fit in u16"));
        if bucket.len() > 64 {
            bucket.pop_front();
        }
    }

    pub(crate) fn get_entries(&self, byte: u8) -> impl Iterator<Item = usize> + '_ {
        self.buckets[usize::from(byte)]
            .iter()
            .copied()
            .map(usize::from)
            .rev()
    }
}
