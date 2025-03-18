use std::io;

use bitter::BitReader;

use crate::block::Block;

use super::huffman::{ASCII_TREE, DISTANCE_TREE, LENGTH_TREE};

pub fn decompress_dcl(input: &Block) -> io::Result<Block> {
    // This follows the implementation from ScummVM, in DecompressorDCL::unpack()
    let input_data = input.read_all()?;
    let input_size = input_data.len();
    let mut reader = bitter::LittleEndianReader::new(&input_data);
    let mut output = Vec::with_capacity(input_size.checked_mul(2).unwrap());
    let Some(mode) = reader.read_u8() else {
        return Err(io::Error::other("Failed to read DCL mode"));
    };
    let Some(dict_type) = reader.read_u8() else {
        return Err(io::Error::other("Failed to read DCL dictionary type"));
    };

    if mode != 0 && mode != 1 {
        return Err(io::Error::other(format!("Unsupported DCL mode: {}", mode)));
    }

    let dict_size = match dict_type {
        4 => 1024,
        5 => 2048,
        6 => 4096,
        _ => {
            return Err(io::Error::other(format!(
                "Unsupported DCL dictionary type: {}",
                dict_type
            )));
        }
    };
    let dict_mask: u32 = dict_size - 1;
    let mut dict = vec![0u8; dict_size as usize];
    let mut dict_pos: u32 = 0;

    loop {
        let should_decode_entry = reader
            .read_bit()
            .ok_or_else(|| io::Error::other("Failed to read DCL entry type"))?;
        if should_decode_entry {
            let length_code = *LENGTH_TREE.lookup(&mut reader)?;
            let token_length = if length_code < 8 {
                (length_code + 2) as u32
            } else {
                let num_bits = (length_code - 7) as u32;
                let extra_bits: u32 = reader
                    .read_bits(num_bits)
                    .ok_or_else(|| io::Error::other("Failed to read DCL extra length bits"))?
                    .try_into()
                    .unwrap();

                8 + (1 << num_bits) + extra_bits
            };

            if token_length == 519 {
                break;
            }

            let distance_code = *DISTANCE_TREE.lookup(&mut reader)? as u32;
            let token_offset: u32 =
                1 + if token_length == 2 {
                    (distance_code << 2)
                        | reader.read_bits(2).ok_or_else(|| {
                            io::Error::other("Failed to read DCL extra distance bits")
                        })? as u32
                } else {
                    (distance_code << dict_type)
                        | reader.read_bits(dict_type as u32).ok_or_else(|| {
                            io::Error::other("Failed to read DCL extra distance bits")
                        })? as u32
                };
            if output.len() < token_offset as usize {
                return Err(io::Error::other("DCL token offset exceeds bytes written"));
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
                reader
                    .read_u8()
                    .ok_or_else(|| io::Error::other("Failed to read DCL byte"))?
            };
            output.push(value);
            dict[dict_pos as usize] = value;
            dict_pos += 1;
            if dict_pos >= dict_size {
                dict_pos = 0;
            }
        }
    }

    Ok(Block::from_vec(output))
}
