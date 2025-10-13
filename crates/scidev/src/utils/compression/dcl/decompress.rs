use std::io;

use crate::utils::compression::reader::{BitReader, LittleEndianReader};

use crate::utils::{block::MemBlock, compression::errors::UnexpectedEndOfInput};

use super::{
    header::{self, CompressionHeader, CompressionMode},
    trees::{ASCII_TREE, DISTANCE_TREE, LENGTH_TREE},
};

#[derive(Debug, thiserror::Error)]
pub enum DecompressionError {
    #[error("Header error: {0}")]
    HeaderDataError(#[from] header::DecodeError),
    #[error("Entry error: {0}")]
    EntryDataError(String),
    #[error(transparent)]
    UnexpectedEndOfInput(#[from] UnexpectedEndOfInput),
    #[error("Inconsistent data: {0}")]
    InconsistentDataError(String),
    #[error(transparent)]
    ReaderError(#[from] io::Error),
}

// A simple write wrapper that counts the number of bytes written.
struct CountWriter<W> {
    inner: W,
    count: usize,
}

impl<W: io::Write> CountWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner, count: 0 }
    }

    fn len(&self) -> usize {
        self.count
    }
}

impl<W: io::Write> io::Write for CountWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = self.inner.write(buf)?;
        self.count += bytes_written;
        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn read_token_length<R: BitReader>(reader: &mut R) -> Result<u32, DecompressionError> {
    let length_code = u32::from(*LENGTH_TREE.lookup(reader)?);
    let token_length = if length_code < 8 {
        length_code + 2
    } else {
        let num_extra_bits = length_code - 7;

        let extra_bits = reader.read_bits(num_extra_bits)?;

        u32::try_from(8 + ((1 << num_extra_bits) | extra_bits)).unwrap()
    };
    Ok(token_length)
}

fn read_token_offset<R: BitReader>(
    header: CompressionHeader,
    token_length: u32,
    reader: &mut R,
) -> Result<usize, DecompressionError> {
    let distance_code = u64::from(*DISTANCE_TREE.lookup(reader)?);
    let num_extra_bits = if token_length == 2 {
        2
    } else {
        u32::from(header.dict_type().num_extra_bits())
    };
    let extra_bits = reader.read_bits(num_extra_bits)?;
    let token_offset = 1 + ((distance_code << num_extra_bits) | extra_bits);

    Ok(usize::try_from(token_offset).unwrap())
}

fn write_dict_entry<W: io::Write>(
    dict: &mut DecompressDictionary,
    token_offset: usize,
    token_length: u32,
    output: &mut W,
) -> io::Result<()> {
    let mut cursor = dict.new_cursor_at_offset(token_offset);

    for _ in 0..token_length {
        output.write_all(&[cursor.next_value()])?;
    }
    Ok(())
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

fn decode_entry<R: BitReader, W: io::Write>(
    header: CompressionHeader,
    reader: &mut R,
    dict: &mut DecompressDictionary,
    output: &mut CountWriter<W>,
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

    write_dict_entry(dict, token_offset, token_length, output)?;
    Ok(LoopAction::Continue)
}

fn decode_byte<R: BitReader, W: io::Write>(
    header: CompressionHeader,
    reader: &mut R,
    dict: &mut DecompressDictionary,
    output: &mut W,
) -> Result<LoopAction, DecompressionError> {
    let value = match header.mode() {
        CompressionMode::Ascii => *ASCII_TREE.lookup(reader)?,
        CompressionMode::Binary => reader.read_u8()?,
    };
    output.write_all(&[value])?;
    dict.push_value(value);
    Ok(LoopAction::Continue)
}

fn decompress_to<R: BitReader, W: io::Write>(
    reader: &mut R,
    output: &mut CountWriter<W>,
) -> Result<(), DecompressionError> {
    let header = CompressionHeader::from_bits(reader)?;

    let mut dict = DecompressDictionary::new(header.dict_type().dict_size());

    loop {
        let should_decode_entry = reader.read_bit()?;
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

#[expect(dead_code, reason = "Will be used in LazyBlock implementation")]
pub(super) fn decompress_dcl_to<R, W>(from: R, to: W) -> Result<(), DecompressionError>
where
    R: io::Read,
    W: io::Write,
{
    let mut reader = LittleEndianReader::new(from);
    decompress_to(&mut reader, &mut CountWriter::new(to))?;
    Ok(())
}

pub fn decompress_dcl(input: &MemBlock) -> Result<MemBlock, DecompressionError> {
    // This follows the implementation from ScummVM, in DecompressorDCL::unpack()
    let input_size = input.size();
    let input_data = input.read_all();
    let mut reader = LittleEndianReader::new(io::Cursor::new(&input_data));
    let mut output = Vec::with_capacity(input_size.checked_mul(2).unwrap());
    decompress_to(
        &mut reader,
        &mut CountWriter::new(io::Cursor::new(&mut output)),
    )?;
    Ok(MemBlock::from_vec(output))
}
