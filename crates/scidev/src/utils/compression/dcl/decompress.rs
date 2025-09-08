use bitter::{BitReader, LittleEndianReader};

use crate::utils::{block::MemBlock, compression::errors::UnexpectedEndOfInput};

use super::trees::{ASCII_TREE, DISTANCE_TREE, LENGTH_TREE};

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

    fn dict_size(self) -> usize {
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

fn read_token_offset<R: BitReader>(
    header: CompressionHeader,
    token_length: u32,
    reader: &mut R,
) -> Result<usize, DecompressionError> {
    let distance_code = usize::from(*DISTANCE_TREE.lookup(reader)?);
    let token_offset: usize = 1 + if token_length == 2 {
        (distance_code << 2)
            | usize::try_from(reader.read_bits(2).ok_or(UnexpectedEndOfInput)?).unwrap()
    } else {
        (distance_code << header.dict_type())
            | usize::try_from(
                reader
                    .read_bits(u32::from(header.dict_type()))
                    .ok_or(UnexpectedEndOfInput)?,
            )
            .unwrap()
    };

    Ok(token_offset)
}

fn write_dict_entry(
    dict: &mut DecompressDictionary,
    token_offset: usize,
    token_length: u32,
    output: &mut Vec<u8>,
) {
    let mut cursor = dict.new_cursor_at_offset(token_offset);

    for _ in 0..token_length {
        output.push(cursor.next_value());
    }
}

struct DecompressDictionary {
    data: Vec<u8>,
    pos: usize,
    mask: usize,
}

impl DecompressDictionary {
    fn new(size: usize) -> Self {
        let mask = size - 1;
        Self {
            data: vec![0u8; size],
            pos: 0,
            mask,
        }
    }

    fn push_value(&mut self, value: u8) {
        self.data[self.pos] = value;
        self.pos = (self.pos + 1) & self.mask;
    }

    fn new_cursor_at_offset(&mut self, offset: usize) -> DecompressDictionaryCursor<'_> {
        let curr_index = (self.pos.wrapping_sub(offset)) & self.mask;
        DecompressDictionaryCursor {
            dict: self,
            curr_index,
        }
    }
}

struct DecompressDictionaryCursor<'a> {
    dict: &'a mut DecompressDictionary,
    curr_index: usize,
}

impl DecompressDictionaryCursor<'_> {
    fn next_value(&mut self) -> u8 {
        let curr_byte = self.dict.data[self.curr_index];
        self.dict.push_value(curr_byte);
        self.curr_index = (self.curr_index + 1) & self.dict.mask;

        // Previously, this code used to wrap back to the start of the copied data when
        // it reached the previous end of the dictionary, however this does not seem to
        // be necessary. As we are copying the values from the same head, when it
        // reaches the end, it will begin copying values that it was previously written. This
        // seems to only cause a difference if we loop the dictionary, in which case we may
        // start overwriting old data, which would be incorrect.

        curr_byte
    }
}

enum LoopAction {
    Stop,
    Continue,
}

fn decode_entry<R: BitReader>(
    header: CompressionHeader,
    reader: &mut R,
    dict: &mut DecompressDictionary,
    output: &mut Vec<u8>,
) -> Result<LoopAction, DecompressionError> {
    let token_length = read_token_length(reader)?;

    if token_length == 519 {
        return Ok(LoopAction::Stop);
    }

    let token_offset = read_token_offset(header, token_length, reader)?;
    if output.len() < token_offset as usize {
        return Err(DecompressionError::InconsistentDataError(
            "DCL token offset exceeds bytes written".into(),
        ));
    }

    eprintln!("token_size_shape: {token_length}, {token_offset}");
    write_dict_entry(dict, token_offset, token_length, output);
    Ok(LoopAction::Continue)
}

fn decode_byte<R: BitReader>(
    header: CompressionHeader,
    reader: &mut R,
    dict: &mut DecompressDictionary,
    output: &mut Vec<u8>,
) -> Result<LoopAction, DecompressionError> {
    let value = if header.mode() == 1 {
        *ASCII_TREE.lookup(reader)?
    } else {
        reader.read_u8().ok_or(UnexpectedEndOfInput)?
    };
    output.push(value);
    dict.push_value(value);
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
            decode_entry(header, reader, &mut dict, output)?
        } else {
            decode_byte(header, reader, &mut dict, output)?
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
