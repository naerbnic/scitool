use std::{collections::VecDeque, io};

/// Provides access to a slice of incoming input data, with the ability to
/// look ahead up to a certain number of bytes.
pub struct InputBuffer<R> {
    reader: R,
    buffer: VecDeque<u8>,
    lookahead: usize,
    closed: bool,
}

impl<R: io::Read> InputBuffer<R> {
    pub fn new(reader: R, lookahead: usize) -> Self {
        // Make the buffer at least twice as large as the lookahead to
        // prevent continuous copying when making the VecDeque contiguous.
        let buffer = VecDeque::with_capacity(lookahead * 2);
        Self {
            reader,
            buffer,
            lookahead,
            closed: false,
        }
    }

    pub fn fill_buffer(&mut self) -> io::Result<()> {
        let mut lookahead_needed = self.lookahead - self.buffer.len();
        let mut read_buffer = [0u8; 1024];
        while !self.closed && lookahead_needed > 0 {
            let to_read = lookahead_needed.min(read_buffer.len());
            assert!(to_read > 0);
            let bytes_read = self.reader.read(&mut read_buffer[..to_read])?;
            if bytes_read == 0 {
                self.closed = true;
                break;
            }
            self.buffer.extend(&read_buffer[..bytes_read]);
            lookahead_needed -= bytes_read;
        }
        assert!(self.buffer.len() <= self.lookahead);
        self.buffer.make_contiguous();
        return Ok(());
    }

    pub fn get_buffer(&self) -> &[u8] {
        let (front_slice, back_slice) = &self.buffer.as_slices();
        // We should always be left in a contiguous state.
        assert!(back_slice.is_empty());
        front_slice
    }

    pub fn consume(&mut self, count: usize) {
        assert!(count <= self.buffer.len());
        self.buffer.drain(0..count);
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty() && self.closed
    }
}
