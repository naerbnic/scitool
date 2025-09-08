use bitter::{BitReader, LittleEndianReader};

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

#[derive(Debug, Clone, Copy)]
struct CompressionHeader {
    mode: u8,
    dict_type: u8,
}

impl CompressionHeader {
    fn from_bits<R: BitReader>(reader: &mut R) -> Result<Self, DecompressionError> {
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

        if !matches!(dict_type, 4..=6) {
            return Err(DecompressionError::HeaderDataError(format!(
                "Unsupported DCL dictionary type: {dict_type}"
            )));
        }

        Ok(CompressionHeader { mode, dict_type })
    }

    fn mode(self) -> u8 {
        self.mode
    }

    fn dict_type(self) -> u8 {
        self.dict_type
    }

    fn dict_size(self) -> u32 {
        match self.dict_type {
            4 => 1024,
            5 => 2048,
            6 => 4096,
            _ => unreachable!("dict_type should have been validated"),
        }
    }
}

fn read_token_length<R: BitReader>(reader: &mut R) -> Result<u32, DecompressionError> {
    let length_code = *LENGTH_TREE.lookup(reader)?;
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
    Ok(token_length)
}

struct DecompressDictionary {
    data: Vec<u8>,
    pos: u32,
    mask: u32,
}

impl DecompressDictionary {
    fn new(size: u32) -> Self {
        let mask = size - 1;
        Self {
            data: vec![0u8; size as usize],
            pos: 0,
            mask,
        }
    }

    fn push_value(&mut self, value: u8) {
        self.data[self.pos as usize] = value;
        self.pos = (self.pos + 1) & self.mask;
    }

    fn new_cursor_at_offset(&mut self, offset: u32) -> DecompressDictionaryCursor<'_> {
        let base_index = (self.pos.wrapping_sub(offset)) & self.mask;
        let curr_index = base_index;
        let next_index = self.pos;
        DecompressDictionaryCursor {
            dict: self,
            base_index,
            curr_index,
            next_index,
        }
    }
}

struct DecompressDictionaryCursor<'a> {
    dict: &'a mut DecompressDictionary,
    base_index: u32,
    curr_index: u32,
    next_index: u32,
}

impl DecompressDictionaryCursor<'_> {
    fn next_value(&mut self) -> u8 {
        let curr_byte = self.dict.data[self.curr_index as usize];
        self.dict.data[self.next_index as usize] = curr_byte;
        self.next_index = (self.next_index + 1) & self.dict.mask;
        self.curr_index = (self.curr_index + 1) & self.dict.mask;
        if self.curr_index == self.dict.pos {
            self.curr_index = self.base_index;
        }

        if self.next_index == self.dict.data.len().try_into().unwrap() {
            self.next_index = 0;
        }
        self.dict.pos = self.next_index;
        curr_byte
    }
}

enum LoopAction {
    Stop,
    Continue,
}

fn decode_entry<R: BitReader>(
    reader: &mut R,
    output: &mut Vec<u8>,
    dict: &mut DecompressDictionary,
    header: CompressionHeader,
) -> Result<LoopAction, DecompressionError> {
    let token_length = read_token_length(reader)?;

    if token_length == 519 {
        return Ok(LoopAction::Stop);
    }

    let distance_code = u32::from(*DISTANCE_TREE.lookup(reader)?);
    let token_offset: u32 = 1 + if token_length == 2 {
        (distance_code << 2)
            | u32::try_from(reader.read_bits(2).ok_or(UnexpectedEndOfInput)?).unwrap()
    } else {
        (distance_code << header.dict_type())
            | u32::try_from(
                reader
                    .read_bits(u32::from(header.dict_type()))
                    .ok_or(UnexpectedEndOfInput)?,
            )
            .unwrap()
    };
    if output.len() < token_offset as usize {
        return Err(DecompressionError::InconsistentDataError(
            "DCL token offset exceeds bytes written".into(),
        ));
    }
    let mut cursor = dict.new_cursor_at_offset(token_offset);

    for _ in 0..token_length {
        output.push(cursor.next_value());
    }
    Ok(LoopAction::Continue)
}

fn decompress_to<R: BitReader>(
    reader: &mut R,
    output: &mut Vec<u8>,
) -> Result<(), DecompressionError> {
    let header = CompressionHeader::from_bits(reader)?;

    let mut dict = DecompressDictionary::new(header.dict_size());

    loop {
        let should_decode_entry = reader.read_bit().ok_or(UnexpectedEndOfInput)?;
        let action = if should_decode_entry {
            decode_entry(reader, output, &mut dict, header)?
        } else {
            let value = if header.mode() == 1 {
                *ASCII_TREE.lookup(reader)?
            } else {
                reader.read_u8().ok_or(UnexpectedEndOfInput)?
            };
            output.push(value);
            dict.push_value(value);
            LoopAction::Continue
        };

        if let LoopAction::Stop = action {
            break;
        }
    }
    Ok(())
}

pub fn decompress_dcl(input: &MemBlock) -> Result<MemBlock, DecompressionError> {
    // This follows the implementation from ScummVM, in DecompressorDCL::unpack()
    let input_size = input.size();
    let input_data = input.read_all();
    let mut reader = LittleEndianReader::new(&input_data);
    let mut output = Vec::with_capacity(input_size.checked_mul(2).unwrap());
    decompress_to(&mut reader, &mut output)?;
    Ok(MemBlock::from_vec(output))
}
