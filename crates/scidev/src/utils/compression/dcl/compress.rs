mod dictionary;
mod index_cache;
mod input_buffer;

use std::io;

use futures::{AsyncRead, AsyncWrite};

use crate::utils::compression::{
    bits::Bits,
    dcl::compress::dictionary::MatchLengthParams,
    pipe::DataProcessor,
    writer::{BitWriter, LittleEndianWriter},
};

use super::{
    header::{CompressionMode, DictType},
    trees::{ASCII_TREE, DISTANCE_TREE, LENGTH_TREE},
};

use self::dictionary::{BackrefMatch, Dictionary};

const MAX_SHORT_BACKREF_OFFSET: usize = 255;
const MAX_BACKREF_LENGTH: usize = 518;

async fn write_token_length<W: BitWriter>(writer: &mut W, length: u32) -> io::Result<()> {
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
        .unwrap_or_else(|| panic!("Length code should be in the tree: {length_code}"));
    length_code_bits.write_to(writer).await?;
    if let Some(extra_bits) = extra_bits {
        extra_bits.write_to(writer).await?;
    }
    Ok(())
}

async fn write_token_offset<W: BitWriter>(
    dict_type: DictType,
    writer: &mut W,
    token_length: usize,
    token_offset: usize,
) -> io::Result<()> {
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
        .unwrap_or_else(|| panic!("Distance code should be in the tree: {distance_code}"));
    distance_code_bits.write_to(writer).await?;
    extra_bits.write_to(writer).await?;
    Ok(())
}

const DEFAULT_PARAMS: MatchLengthParams = MatchLengthParams {
    short_max_offset: MAX_SHORT_BACKREF_OFFSET,
    max: MAX_BACKREF_LENGTH,
    min: 2,
    sufficient: Some(18),
};

async fn compress_dcl_to<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    match_params: MatchLengthParams,
    mode: CompressionMode,
    dict_type: DictType,
    input: R,
    writer: W,
) -> io::Result<()> {
    let mut writer = LittleEndianWriter::new(writer);
    let mut dict = Dictionary::new(dict_type);
    let mut input_buffer = input_buffer::InputBuffer::new(input, match_params.max);
    input_buffer.fill_buffer().await?;
    mode.write_to(&mut writer).await?;
    dict_type.write_to(&mut writer).await?;
    while !input_buffer.is_empty() {
        let curr_buffer = input_buffer.get_buffer();
        let num_bytes_consumed = if let Some(BackrefMatch { offset, length }) =
            dict.find_best_match(&DEFAULT_PARAMS, curr_buffer)
            && length >= 2
        {
            // Write a back-reference token
            writer.write_bit(true).await?;
            write_token_length(&mut writer, u32::try_from(length).unwrap()).await?;
            write_token_offset(dict_type, &mut writer, length, offset).await?;
            length
        } else {
            // Write a literal token
            writer.write_bit(false).await?;
            let byte = curr_buffer[0];
            match mode {
                CompressionMode::Ascii => {
                    // We use the ASCII tree for mode 1
                    let encoding = ASCII_TREE
                        .encoding_of(&byte)
                        .expect("Byte should be in the tree");
                    encoding.write_to(&mut writer).await?;
                }
                CompressionMode::Binary => {
                    // We use a raw byte for mode 0
                    writer.write_u8(byte).await?;
                }
            }
            1
        };

        dict.append_data(&curr_buffer[..num_bytes_consumed]);
        input_buffer.consume(num_bytes_consumed);
        input_buffer.fill_buffer().await?;
    }

    // Write the terminator, which is a back-reference of length 519
    writer.write_bit(true).await?;
    write_token_length(&mut writer, 519).await?;

    writer
        .write_bits(writer.bits_until_byte_aligned(), 0u64)
        .await?;
    writer.finish().await?;
    Ok(())
}

#[derive(Debug, Clone)]
struct CompressDclProcessor {
    match_params: MatchLengthParams,
    mode: CompressionMode,
    dict_type: DictType,
}

impl CompressDclProcessor {
    pub(crate) fn new(mode: CompressionMode, dict_type: DictType) -> Self {
        Self {
            match_params: DEFAULT_PARAMS,
            mode,
            dict_type,
        }
    }
}

impl DataProcessor for CompressDclProcessor {
    async fn process<R, W>(self, reader: R, writer: W) -> Result<(), io::Error>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        compress_dcl_to(self.match_params, self.mode, self.dict_type, reader, writer).await
    }
}

pub fn compress_dcl(
    mode: CompressionMode,
    dict_type: DictType,
    input: &[u8],
    output: &mut Vec<u8>,
) -> io::Result<()> {
    let processor = CompressDclProcessor::new(mode, dict_type);
    processor.process_sync(input, output)
}
