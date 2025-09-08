use super::writer::BitWriter;

#[derive(Debug, Clone, Copy)]
pub(super) struct Bits {
    value: u64,
    num_bits: u8,
}

impl Bits {
    pub(super) fn empty() -> Self {
        Self {
            value: 0,
            num_bits: 0,
        }
    }

    pub(super) fn append_bit(self, bit: bool) -> Self {
        let bit_mask = u64::from(bit) << self.num_bits;
        let value = self.value | bit_mask;
        Self {
            value,
            num_bits: self.num_bits + 1,
        }
    }

    pub(super) fn from_le_bits<T, B>(value: T, num_bits: B) -> Self
    where
        T: Into<u64>,
        B: Into<u64>,
    {
        let num_bits = num_bits.into();
        assert!(num_bits <= 64);
        let num_bits = u8::try_from(num_bits).unwrap();
        Self {
            value: value.into() & ((1 << num_bits) - 1),
            num_bits,
        }
    }

    pub(super) fn write_to<W: BitWriter>(&self, writer: &mut W) {
        writer.write_bits(self.num_bits, self.value);
    }
}
