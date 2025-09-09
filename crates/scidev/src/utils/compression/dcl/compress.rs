mod dictionary;

use crate::utils::compression::{
    bits::Bits,
    dcl::compress::dictionary::MatchLengthParams,
    writer::{BitWriter, LittleEndianWriter},
};

use super::{
    header::{CompressionMode, DictType},
    trees::{ASCII_TREE, DISTANCE_TREE, LENGTH_TREE},
};

use self::dictionary::{BackrefMatch, Dictionary};

const MAX_BACKREF_LENGTH: usize = 518;

fn write_token_length<W: BitWriter>(writer: &mut W, length: u32) {
    let (length_code, extra_bits) = if length < 10 {
        assert!(length >= 2);
        (length - 2, None::<Bits>)
    } else {
        let length = length - 8;
        // From the little end, we want to know the position of the highest set bit.
        // Since the initial value was at least 10, then we know that we will have at
        // least one bit set at an index of 2 or higher.
        let first_set_bit = 32 - length.leading_zeros();
        let num_extra_bits = first_set_bit - 1;
        let bits = Bits::from_le_bits(length, num_extra_bits);
        (7 + num_extra_bits, Some(bits))
    };
    let length_code_bits = LENGTH_TREE
        .encoding_of(&u8::try_from(length_code).unwrap())
        .expect("Length code should be in the tree");
    length_code_bits.write_to(writer);
    if let Some(extra_bits) = extra_bits {
        extra_bits.write_to(writer);
    }
}

fn write_token_offset<W: BitWriter>(
    dict_type: DictType,
    writer: &mut W,
    token_length: usize,
    token_offset: usize,
) {
    assert!(token_offset > 0);
    let encoding = token_offset - 1;

    let num_extra_bits = if token_length == 2 {
        2
    } else {
        usize::from(dict_type.num_extra_bits())
    };

    let extra_bits = Bits::from_le_bits(
        u64::try_from(encoding).unwrap(),
        u64::try_from(num_extra_bits).unwrap(),
    );

    let distance_code = encoding >> num_extra_bits;
    let distance_code_bits = DISTANCE_TREE
        .encoding_of(&u8::try_from(distance_code).unwrap())
        .expect("Distance code should be in the tree");
    distance_code_bits.write_to(writer);
    extra_bits.write_to(writer);
}

const DEFAULT_PARAMS: MatchLengthParams = MatchLengthParams {
    max: MAX_BACKREF_LENGTH,
    min: 2,
    sufficient: Some(18),
};

pub fn compress_dcl(
    mode: CompressionMode,
    dict_type: DictType,
    mut input: &[u8],
    output: &mut Vec<u8>,
) {
    let mut writer = LittleEndianWriter::new(output);
    let mut dict = Dictionary::new(dict_type);
    mode.write_to(&mut writer);
    dict_type.write_to(&mut writer);
    while !input.is_empty() {
        let num_bytes_consumed = if let Some(BackrefMatch { offset, length }) =
            dict.find_best_match(&DEFAULT_PARAMS, input)
            && length >= 2
        {
            let length = length.min(MAX_BACKREF_LENGTH);
            // Write a back-reference token
            writer.write_bit(true);
            write_token_length(&mut writer, u32::try_from(length).unwrap());
            write_token_offset(dict_type, &mut writer, length, offset);
            length
        } else {
            // Write a literal token
            writer.write_bit(false);
            let byte = input[0];
            match mode {
                CompressionMode::Ascii => {
                    // We use the ASCII tree for mode 1
                    let encoding = ASCII_TREE
                        .encoding_of(&byte)
                        .expect("Byte should be in the tree");
                    encoding.write_to(&mut writer);
                }
                CompressionMode::Binary => {
                    // We use a raw byte for mode 0
                    writer.write_u8(byte);
                }
            }
            1
        };

        dict.append_data(&input[..num_bytes_consumed]);
        input = &input[num_bytes_consumed..];
    }

    // Write the terminator, which is a back-reference of length 519
    writer.write_bit(true);
    write_token_length(&mut writer, 519);

    writer.write_bits(writer.bits_until_byte_aligned(), 0u64);
}
