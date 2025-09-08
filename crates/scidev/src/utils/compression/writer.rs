pub(super) trait BitWriter {
    /// Write a single bit to the output.
    fn write_bit(&mut self, bit: bool);

    /// Write a full byte to the output.
    fn write_u8(&mut self, byte: u8);

    /// Writes `count` lower bits of 'bits' to the output.
    fn write_bits(&mut self, count: u8, bits: u64);

    /// Returns the number of bits that need to be written to reach the next byte boundary.
    fn bits_until_byte_aligned(&self) -> u8;
}

pub(super) struct LittleEndianWriter<'a> {
    output: &'a mut Vec<u8>,
    curr_byte: u8,
    bits_filled: u8,
}

impl<'a> LittleEndianWriter<'a> {
    pub(super) fn new(output: &'a mut Vec<u8>) -> Self {
        LittleEndianWriter {
            output,
            curr_byte: 0,
            bits_filled: 0,
        }
    }
}

impl BitWriter for LittleEndianWriter<'_> {
    fn write_bit(&mut self, bit: bool) {
        if bit {
            self.curr_byte |= 1 << self.bits_filled;
        }
        self.bits_filled += 1;
        if self.bits_filled == 8 {
            self.output.push(self.curr_byte);
            self.curr_byte = 0;
            self.bits_filled = 0;
        }
    }

    fn write_u8(&mut self, byte: u8) {
        self.write_bits(8, u64::from(byte));
    }

    fn write_bits(&mut self, count: u8, bits: u64) {
        assert!(count <= 64);
        for i in 0..count {
            let bit = (bits >> i) & 1 != 0;
            self.write_bit(bit);
        }
    }

    fn bits_until_byte_aligned(&self) -> u8 {
        if self.bits_filled == 0 {
            0
        } else {
            8 - self.bits_filled
        }
    }
}

impl Drop for LittleEndianWriter<'_> {
    fn drop(&mut self) {
        if self.bits_filled > 0 {
            self.output.push(self.curr_byte);
        }
    }
}
