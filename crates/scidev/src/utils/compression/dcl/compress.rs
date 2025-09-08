use crate::utils::compression::writer::BitWriter;

#[expect(dead_code, reason = "In process of being implemented")]
fn write_token_length<W: BitWriter>(writer: &mut W, length: u32) {
    if length < 10 {
        writer.write_bits(3, u64::from(length - 2));
    } else {
        let mut length_code: u32 = 8;
        let mut len = length - 8;
        while len > 1 {
            len >>= 1;
            length_code += 1;
        }
        let num_extra_bits: u8 = u8::try_from(length_code - 7).unwrap();
        let extra_bits = length - (1 << (length_code - 7)) - 8;
        writer.write_bits(3, u64::from(length_code));
        writer.write_bits(num_extra_bits, u64::from(extra_bits));
    }
}
