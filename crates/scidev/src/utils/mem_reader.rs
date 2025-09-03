use std::{any::TypeId, backtrace::BacktraceStatus, borrow::Cow, mem::MaybeUninit, ops::Bound};

use crate::utils::buffer::{Buffer, BufferError, BufferExt, FromFixedBytes};

fn convert_if_different<T, Target, F>(value: T, convert: F) -> Target
where
    T: 'static,
    Target: 'static,
    F: FnOnce(T) -> Target,
{
    if TypeId::of::<T>() == TypeId::of::<Target>() {
        // SAFETY: We just checked that T and Target are the same type.
        let mut value = MaybeUninit::new(value);
        #[allow(unsafe_code)]
        unsafe {
            value.as_mut_ptr().cast::<Target>().read()
        }
    } else {
        convert(value)
    }
}

#[derive(Debug)]
pub struct Context {
    start: usize,
    end: usize,
    message: Option<String>,
}

#[derive(Debug)]
enum Contents {
    Message(String),
    Error(Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug)]
pub struct Error {
    backtrace: std::backtrace::Backtrace,
    contexts: Vec<Context>,
    contents: Contents,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.contents {
            Contents::Message(msg) => write!(f, "{msg}")?,
            Contents::Error(err) => write!(f, "Error: {err}")?,
        }

        for context in &self.contexts {
            if let Some(msg) = &context.message {
                write!(f, " at [{}..{}] ({})", context.start, context.end, msg)?;
            } else {
                write!(f, " at [{}..{}]", context.start, context.end)?;
            }
        }
        if let BacktraceStatus::Captured = self.backtrace.status() {
            write!(f, "\n\nBacktrace:\n{}", self.backtrace)?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.contents {
            Contents::Message(_) => None,
            Contents::Error(err) => Some(err.as_ref()),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct MemReader<'a, B: 'a> {
    buffer: &'a B,
    start: usize,
    end: usize,
    position: usize,
    context: ReaderContext<'a, B>,
}

impl<'a, B: 'a> std::fmt::Debug for MemReader<'a, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemReader")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("position", &self.position)
            .finish()
    }
}

#[derive(Debug, Clone)]
enum ReaderContext<'a, B> {
    Root,
    Prev {
        prev_reader: &'a MemReader<'a, B>,
        context: Option<Cow<'a, str>>,
    },
}

impl<'a, B: 'a> MemReader<'a, B>
where
    B: Buffer,
{
    fn check_read_length(&self, len: usize) -> Result<()> {
        let remaining = self.end - self.start - self.position;
        if remaining < len {
            return Err(self.err_with_context()(BufferError::NotEnoughData {
                required: len,
                available: remaining,
            }));
        }
        Ok(())
    }

    fn make_context(&self) -> Vec<Context> {
        let mut contexts = Vec::new();
        let mut curr_reader = self;
        loop {
            match &curr_reader.context {
                ReaderContext::Root => {
                    contexts.push(Context {
                        start: curr_reader.start,
                        end: curr_reader.end,
                        message: None,
                    });
                    return contexts;
                }
                ReaderContext::Prev {
                    prev_reader,
                    context,
                } => {
                    contexts.push(Context {
                        start: curr_reader.start - prev_reader.start,
                        end: curr_reader.end - prev_reader.start,
                        message: context.as_ref().map(|s| s.clone().into_owned()),
                    });
                    curr_reader = prev_reader;
                }
            }
        }
    }

    #[track_caller]
    #[allow(unsafe_code)]
    fn err_with_context<E>(&self) -> impl FnOnce(E) -> Error
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        move |err| {
            convert_if_different(err, |err| Error {
                backtrace: std::backtrace::Backtrace::capture(),
                contexts: self.make_context(),
                contents: Contents::Error(Box::new(err)),
            })
        }
    }

    #[expect(dead_code)]
    #[track_caller]
    fn err_with_message(&self, message: String) -> Error {
        Error {
            backtrace: std::backtrace::Backtrace::capture(),
            contexts: self.make_context(),
            contents: Contents::Message(message),
        }
    }

    fn make_sub_reader<'new>(
        &'new self,
        start: usize,
        end: usize,
        context: Option<Cow<'new, str>>,
    ) -> MemReader<'new, B> {
        let new_start = self.start + start;
        let new_end = self.start + end;
        MemReader {
            buffer: self.buffer,
            start: new_start,
            end: new_end,
            position: 0,
            context: ReaderContext::Prev {
                prev_reader: self,
                context,
            },
        }
    }

    fn get_slice(&self, len: usize) -> Result<&'a [u8]> {
        self.check_read_length(len)?;
        let buffer_data = &self.buffer.as_slice()[self.start + self.position..][..len];
        Ok(buffer_data)
    }

    fn get_whole_slice(&self) -> &'a [u8] {
        self.get_slice(self.remaining()).unwrap()
    }

    pub fn new(buf: &'a B) -> Self {
        Self {
            buffer: buf,
            start: 0,
            end: buf.size(),
            position: 0,
            context: ReaderContext::Root,
        }
    }

    pub fn sub_reader_range<'b, R, Ctxt>(
        &'b self,
        context: Ctxt,
        range: R,
    ) -> Result<MemReader<'b, B>>
    where
        R: std::ops::RangeBounds<usize>,
        Ctxt: Into<Cow<'b, str>>,
    {
        let start_bound: Bound<&usize> = range.start_bound();
        let end_bound: Bound<&usize> = range.end_bound();

        let given_start = match start_bound {
            Bound::Included(&start) => Some(start),
            Bound::Excluded(&start) => Some(start + 1),
            Bound::Unbounded => None,
        };

        let given_end = match end_bound {
            Bound::Included(&end) => Some(end + 1),
            Bound::Excluded(&end) => Some(end),
            Bound::Unbounded => None,
        };

        if let Some(start) = given_start
            && let Some(end) = given_end
        {
            assert!(start <= end);
        }

        let start = given_start.unwrap_or(0);
        let end = given_end.unwrap_or(self.remaining());

        if start > end {
            // This must have been caused by an implicit range endpoint, so treat
            // it as an error, not a panic.
            return Err(self.err_with_context()(BufferError::NotEnoughData {
                required: start,
                available: end,
            }));
        }

        Ok(self.make_sub_reader(
            self.position + start,
            self.position + end,
            Some(context.into()),
        ))
    }

    pub fn sub_reader_at_offset_length<'b, Ctxt>(
        &'b self,
        context: Ctxt,
        offset: usize,
        length: Option<usize>,
    ) -> Result<MemReader<'b, B>>
    where
        Ctxt: Into<Cow<'b, str>>,
    {
        self.sub_reader_range(context, offset..offset + length.unwrap_or(self.remaining()))
    }

    pub fn read_exact(&mut self, len: usize) -> Result<&[u8]> {
        let buffer_data = self.get_slice(len)?;
        self.position += len;
        Ok(buffer_data)
    }

    pub fn read_to_subreader<'b, Ctxt>(
        &'b mut self,
        context: Ctxt,
        len: usize,
    ) -> Result<MemReader<'b, B>>
    where
        Ctxt: Into<Cow<'b, str>>,
    {
        self.check_read_length(len)?;
        let old_position = self.position;
        self.position += len;
        let sub_reader =
            self.make_sub_reader(old_position, old_position + len, Some(context.into()));
        Ok(sub_reader)
    }

    pub fn read_remainder_slice(&mut self) -> &'a [u8] {
        self.get_whole_slice()
    }

    #[must_use]
    pub fn into_buffer(self) -> B {
        let buf_start = self.start + self.position;
        let buf_end = self.end;
        self.buffer.clone().sub_buffer(buf_start..buf_end).unwrap()
    }

    /// Reads a value from the front of the buffer, returning the value and the
    /// remaining buffer.
    // Functions that can be implemented in terms of the above functions.
    pub fn read_value<T: FromFixedBytes>(&mut self, context: &str) -> Result<T> {
        let value_data = self.read_to_subreader(context, T::SIZE)?;
        let value = T::parse(value_data.get_whole_slice());
        Ok(value)
    }

    /// Reads N values from the front of the buffer, returning the values and the
    /// remaining buffer.
    pub fn read_values<T: FromFixedBytes>(
        &mut self,
        context: &str,
        count: usize,
    ) -> Result<Vec<T>> {
        let mut values = Vec::with_capacity(count);
        for i in 0..count {
            values.push(self.read_value(&format!("{context}[{i}]"))?);
        }
        Ok(values)
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.end - self.start - self.position
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Splits the block into chunks of the given size. Panics if the block size
    /// is not a multiple of the chunk size.
    pub fn with_chunks<F, R, E>(
        &mut self,
        chunk_size: usize,
        context: &str,
        mut body: F,
    ) -> Result<Vec<R>>
    where
        F: for<'b> FnMut(MemReader<'b, B>) -> std::result::Result<R, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let mut chunks = Vec::new();
        let overflow = self.remaining() % chunk_size;
        if overflow != 0 {
            return Err(self.err_with_context()(BufferError::NotDivisible {
                required: chunk_size,
                overflow,
            }));
        }
        let mut index = 0;
        while !self.is_empty() {
            let entry_context = format!("{context}[{index}]");
            let sub_reader = self.make_sub_reader(
                self.position,
                self.position + chunk_size,
                Some(entry_context.into()),
            );

            chunks.push(body(sub_reader).map_err(self.err_with_context())?);
            self.position += chunk_size;
            index += 1;
        }
        Ok(chunks)
    }

    #[expect(dead_code)]
    fn split_values<T: FromFixedBytes>(&mut self, context: &str) -> Result<Vec<T>> {
        let values = Vec::with_capacity(self.remaining() / T::SIZE);
        self.with_chunks(T::SIZE, context, |mut r| r.read_value::<T>("value"))?;
        Ok(values)
    }

    #[expect(dead_code)]
    fn read_length_delimited_block<'b>(
        &'b mut self,
        context: &'b str,
        item_size: usize,
    ) -> Result<MemReader<'b, B>> {
        let num_blocks = self.read_value::<u16>(&format!("{context}(length)"))?;
        let total_block_size = usize::from(num_blocks)
            .checked_mul(item_size)
            .expect("Size calculation should not overflow");
        self.read_to_subreader(format!("{context}(contents)"), total_block_size)
    }

    /// Reads a sequence of values, where the first value is a little endian
    /// u16 indicating the number of values to read.
    #[expect(dead_code)]
    fn read_length_delimited_records<T: FromFixedBytes>(
        &mut self,
        context: &str,
    ) -> Result<Vec<T>> {
        let num_records = self.read_value::<u16>(&format!("{context}(length)"))?;
        self.read_values::<T>(&format!("{context}(values)"), num_records as usize)
    }

    pub fn seek_to(&mut self, offset: usize) -> Result<()> {
        if self.data_size() < offset {
            return Err(self.err_with_context()(BufferError::NotEnoughData {
                required: offset,
                available: self.data_size(),
            }));
        }

        self.position = offset;
        Ok(())
    }

    #[must_use]
    pub fn tell(&self) -> usize {
        self.position
    }

    #[must_use]
    pub fn data_size(&self) -> usize {
        self.end - self.start
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let buf = self.read_exact(1)?;
        Ok(buf[0])
    }

    pub fn read_u16_le(&mut self) -> Result<u16> {
        let buf = self.read_exact(2)?;
        Ok(u16::from_le_bytes(buf.try_into().unwrap()))
    }

    pub fn read_u24_le(&mut self) -> Result<u32> {
        let buf = self.read_exact(3)?;
        assert!(buf.len() == 3);
        Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], 0]))
    }

    pub fn read_u32_le(&mut self) -> Result<u32> {
        let buf = self.read_exact(4)?;
        Ok(u32::from_le_bytes(buf.try_into().unwrap()))
    }
}
