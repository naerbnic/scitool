use std::io;

use futures::{AsyncRead, AsyncWrite, AsyncWriteExt as _};

use crate::utils::compression::pipe::{self, DataProcessor};
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
    ReaderError(io::Error),
}

impl From<io::Error> for DecompressionError {
    fn from(value: io::Error) -> Self {
        match value.downcast::<Self>() {
            Ok(decomp_err) => decomp_err,
            Err(io_err) => Self::ReaderError(io_err),
        }
    }
}

// A simple write wrapper that counts the number of bytes written.
#[pin_project::pin_project]
struct CountWriter<W> {
    #[pin]
    inner: W,
    count: usize,
}

impl<W: AsyncWrite + Unpin> CountWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner, count: 0 }
    }

    fn len(&self) -> usize {
        self.count
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for CountWriter<W> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let project = self.project();
        match std::task::ready!(project.inner.poll_write(cx, buf)) {
            Ok(n) => {
                *project.count += n;
                std::task::Poll::Ready(Ok(n))
            }
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let project = self.project();
        project.inner.poll_flush(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let project = self.project();
        project.inner.poll_close(cx)
    }
}

async fn read_token_length<R: BitReader>(reader: &mut R) -> Result<u32, DecompressionError> {
    let length_code = u32::from(*LENGTH_TREE.lookup(reader).await?);
    let token_length = if length_code < 8 {
        length_code + 2
    } else {
        let num_extra_bits = length_code - 7;

        let extra_bits = reader.read_bits(num_extra_bits).await?;

        u32::try_from(8 + ((1 << num_extra_bits) | extra_bits)).unwrap()
    };
    Ok(token_length)
}

async fn read_token_offset<R: BitReader>(
    header: CompressionHeader,
    token_length: u32,
    reader: &mut R,
) -> Result<usize, DecompressionError> {
    let distance_code = u64::from(*DISTANCE_TREE.lookup(reader).await?);
    let num_extra_bits = if token_length == 2 {
        2
    } else {
        u32::from(header.dict_type().num_extra_bits())
    };
    let extra_bits = reader.read_bits(num_extra_bits).await?;
    let token_offset = 1 + ((distance_code << num_extra_bits) | extra_bits);

    Ok(usize::try_from(token_offset).unwrap())
}

async fn write_dict_entry<W: AsyncWrite + Unpin>(
    dict: &mut DecompressDictionary,
    token_offset: usize,
    token_length: u32,
    output: &mut W,
) -> io::Result<()> {
    let mut cursor = dict.new_cursor_at_offset(token_offset);

    for _ in 0..token_length {
        output.write_all(&[cursor.next_value()]).await?;
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

async fn decode_entry<R: BitReader, W: AsyncWrite + Unpin>(
    header: CompressionHeader,
    reader: &mut R,
    dict: &mut DecompressDictionary,
    output: &mut CountWriter<W>,
) -> Result<LoopAction, DecompressionError> {
    let token_length = read_token_length(reader).await?;

    if token_length == 519 {
        return Ok(LoopAction::Stop);
    }

    let token_offset = read_token_offset(header, token_length, reader).await?;
    if output.len() < token_offset as usize {
        return Err(DecompressionError::InconsistentDataError(
            "DCL token offset exceeds bytes written".into(),
        ));
    }

    write_dict_entry(dict, token_offset, token_length, output).await?;
    Ok(LoopAction::Continue)
}

async fn decode_byte<R: BitReader, W: AsyncWrite + Unpin>(
    header: CompressionHeader,
    reader: &mut R,
    dict: &mut DecompressDictionary,
    output: &mut W,
) -> Result<LoopAction, DecompressionError> {
    let value = match header.mode() {
        CompressionMode::Ascii => *ASCII_TREE.lookup(reader).await?,
        CompressionMode::Binary => reader.read_u8().await?,
    };
    output.write_all(&[value]).await?;
    dict.push_value(value);
    Ok(LoopAction::Continue)
}

async fn decompress_to<R: BitReader, W: AsyncWrite + Unpin>(
    reader: &mut R,
    output: &mut CountWriter<W>,
) -> Result<(), DecompressionError> {
    let header = CompressionHeader::from_bits(reader).await?;

    let mut dict = DecompressDictionary::new(header.dict_type().dict_size());

    loop {
        let should_decode_entry = reader.read_bit().await?;
        let action = if should_decode_entry {
            decode_entry(header, reader, &mut dict, output).await?
        } else {
            decode_byte(header, reader, &mut dict, output).await?
        };

        if let LoopAction::Stop = action {
            break;
        }
    }
    output.flush().await?;
    // let mut buf: &[u8] = &[0u8; 128];
    // loop {
    //     if reader.
    // }
    Ok(())
}

pub(super) struct DecompressDclProcessor;

impl pipe::DataProcessor for DecompressDclProcessor {
    async fn process<R, W>(self, reader: R, writer: W) -> Result<(), io::Error>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut reader = LittleEndianReader::new(reader);
        decompress_to(&mut reader, &mut CountWriter::new(writer))
            .await
            .map_err(|e| match e {
                DecompressionError::ReaderError(io_err) => io_err,
                other => io::Error::other(other),
            })?;
        Ok(())
    }
}

pub fn decompress_reader<'a, R>(reader: R) -> impl io::Read + 'a
where
    R: io::Read + Unpin + 'a,
{
    DecompressDclProcessor.pull(reader, 8192)
}

pub fn decompress_dcl(input: &MemBlock) -> Result<MemBlock, DecompressionError> {
    // This follows the implementation from ScummVM, in DecompressorDCL::unpack()
    let input_size = input.size();
    let input_data = input.read_all();
    let mut output = Vec::with_capacity(input_size.checked_mul(2).unwrap());
    {
        let mut source = io::Cursor::new(input_data);
        let mut sink = DecompressDclProcessor.push(io::Cursor::new(&mut output), 1024);
        std::io::copy(&mut source, &mut sink)?;
        sink.close()?;
    }
    Ok(MemBlock::from_vec(output))
}
