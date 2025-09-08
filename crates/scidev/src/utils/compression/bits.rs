use bitter::BitReader;

use crate::utils::compression::writer::BitWriter;

#[derive(Debug, Clone, Copy)]
pub struct Bits {
    value: u64,
    num_bits: u8,
}

impl Bits {
    pub fn new(value: u64, num_bits: u8) -> Self {
        Self { value, num_bits }
    }

    pub fn read_from<R: BitReader>(reader: &mut R, num_bits: u8) -> Option<Self> {
        let value = reader.read_bits(u32::from(num_bits))?;
        Some(Self { value, num_bits })
    }

    pub fn write_to<W: BitWriter>(&self, writer: &mut W) {
        writer.write_bits(self.num_bits, self.value);
    }
}
