use bitter::BitReader;

use crate::utils::{block::MemBlock, compression::errors::UnexpectedEndOfInput};

use super::huffman::{ASCII_TREE, DISTANCE_TREE, LENGTH_TREE};

#[derive(Debug, thiserror::Error)]
pub enum DecompressionError {
    #[error("Header error: {0}")]
    HeaderDataError(String),
    #[error("Entry error: {0}")]
    EntryDataError(String),
    #[error(transparent)]
    UnexpectedEndOfInput(#[from] UnexpectedEndOfInput),
    #[error("Inconsistent data: {0}")]
    InconsistentDataError(String),
}

pub fn decompress_dcl(input: &MemBlock) -> Result<MemBlock, DecompressionError> {
    // This follows the implementation from ScummVM, in DecompressorDCL::unpack()
    let input_size = input.size();
    let input_data = input.read_all();
    let mut reader = bitter::LittleEndianReader::new(&input_data);
    let mut output = Vec::with_capacity(input_size.checked_mul(2).unwrap());
    let Some(mode) = reader.read_u8() else {
        return Err(DecompressionError::HeaderDataError(
            "Failed to read DCL mode".into(),
        ));
    };
    let Some(dict_type) = reader.read_u8() else {
        return Err(DecompressionError::HeaderDataError(
            "Failed to read DCL dictionary type".into(),
        ));
    };

    if mode != 0 && mode != 1 {
        return Err(DecompressionError::HeaderDataError(format!(
            "Unsupported DCL mode: {mode}"
        )));
    }

    let dict_size = match dict_type {
        4 => 1024,
        5 => 2048,
        6 => 4096,
        _ => {
            return Err(DecompressionError::HeaderDataError(format!(
                "Unsupported DCL dictionary type: {dict_type}"
            )));
        }
    };
    let dict_mask: u32 = dict_size - 1;
    let mut dict = vec![0u8; dict_size as usize];
    let mut dict_pos: u32 = 0;

    loop {
        let should_decode_entry = reader.read_bit().ok_or(UnexpectedEndOfInput)?;
        if should_decode_entry {
            let length_code = *LENGTH_TREE.lookup(&mut reader)?;
            let token_length = if length_code < 8 {
                u32::from(length_code + 2)
            } else {
                let num_bits = u32::from(length_code - 7);
                let extra_bits: u32 = reader
                    .read_bits(num_bits)
                    .ok_or(UnexpectedEndOfInput)?
                    .try_into()
                    .unwrap();

                8 + (1 << num_bits) + extra_bits
            };

            if token_length == 519 {
                break;
            }

            let distance_code = u32::from(*DISTANCE_TREE.lookup(&mut reader)?);
            let token_offset: u32 = 1 + if token_length == 2 {
                (distance_code << 2)
                    | u32::try_from(reader.read_bits(2).ok_or(UnexpectedEndOfInput)?).unwrap()
            } else {
                (distance_code << dict_type)
                    | u32::try_from(
                        reader
                            .read_bits(u32::from(dict_type))
                            .ok_or(UnexpectedEndOfInput)?,
                    )
                    .unwrap()
            };
            if output.len() < token_offset as usize {
                return Err(DecompressionError::InconsistentDataError(
                    "DCL token offset exceeds bytes written".into(),
                ));
            }

            let base_index = (dict_pos.wrapping_sub(token_offset)) & dict_mask;
            let mut curr_index = base_index;
            let mut next_index = dict_pos;

            for _ in 0..token_length {
                let curr_byte = dict[curr_index as usize];
                output.push(curr_byte);
                dict[next_index as usize] = curr_byte;
                next_index = (next_index + 1) & dict_mask;
                curr_index = (curr_index + 1) & dict_mask;
                if curr_index == dict_pos {
                    curr_index = base_index;
                }

                if next_index == dict_size {
                    next_index = 0;
                }
                dict_pos = next_index;
            }
        } else {
            let value = if mode == 1 {
                *ASCII_TREE.lookup(&mut reader)?
            } else {
                reader.read_u8().ok_or(UnexpectedEndOfInput)?
            };
            output.push(value);
            dict[dict_pos as usize] = value;
            dict_pos += 1;
            if dict_pos >= dict_size {
                dict_pos = 0;
            }
        }
    }

    Ok(MemBlock::from_vec(output))
}
