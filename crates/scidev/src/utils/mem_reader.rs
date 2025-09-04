use std::{
    any::TypeId,
    backtrace::BacktraceStatus,
    borrow::Cow,
    fmt::{Debug, Display},
    mem::MaybeUninit,
    ops::Bound,
};

use crate::utils::{
    buffer::{Buffer, BufferError, FromFixedBytes},
    errors::{BlockContext, InvalidDataError, OtherError},
};

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
pub struct Error {
    backtrace: std::backtrace::Backtrace,
    invalid_data: InvalidDataError<OtherError>,
}

impl Error {
    pub fn new<E>(invalid_data: InvalidDataError<E>) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            backtrace: std::backtrace::Backtrace::capture(),
            invalid_data: invalid_data.map(OtherError::new),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.invalid_data, f)?;
        if let BacktraceStatus::Captured = self.backtrace.status() {
            write!(f, "\n\nBacktrace:\n{}", self.backtrace)?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(&self.invalid_data)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait MemReader<'a>: Clone {
    type Sibling<'b>: MemReader<'b>
    where
        Self: 'b;

    fn seek_to(&mut self, offset: usize) -> Result<()>;

    #[must_use]
    fn tell(&self) -> usize;

    #[must_use]
    fn data_size(&self) -> usize;

    fn read_exact(&mut self, len: usize) -> Result<&[u8]>;

    fn read_remainder_slice(&mut self) -> &'a [u8];

    #[must_use]
    fn remaining(&self) -> usize;

    #[must_use]
    fn is_empty(&self) -> bool;

    fn sub_reader_range<'b, R, Ctxt>(
        &'b self,
        context: Ctxt,
        range: R,
    ) -> Result<Self::Sibling<'b>>
    where
        R: std::ops::RangeBounds<usize>,
        Ctxt: Into<Cow<'b, str>>;

    fn sub_reader_at_offset_length<'b, Ctxt>(
        &'b self,
        context: Ctxt,
        offset: usize,
        length: Option<usize>,
    ) -> Result<Self::Sibling<'b>>
    where
        Ctxt: Into<Cow<'b, str>>,
    {
        self.sub_reader_range(context, offset..offset + length.unwrap_or(self.remaining()))
    }

    fn read_to_subreader<'b, Ctxt>(
        &'b mut self,
        context: Ctxt,
        len: usize,
    ) -> Result<Self::Sibling<'b>>
    where
        Ctxt: Into<Cow<'b, str>>;

    /// Reads a value from the front of the buffer, returning the value and the
    /// remaining buffer.
    // Functions that can be implemented in terms of the above functions.
    fn read_value<T: FromFixedBytes>(&mut self, context: &str) -> Result<T> {
        let mut value_data = self.read_to_subreader(context.to_owned(), T::SIZE)?;
        let value = T::parse(value_data.read_remainder_slice());
        Ok(value)
    }

    /// Reads N values from the front of the buffer, returning the values and the
    /// remaining buffer.
    fn read_values<T: FromFixedBytes>(&mut self, context: &str, count: usize) -> Result<Vec<T>> {
        let mut values = Vec::with_capacity(count);
        for i in 0..count {
            values.push(self.read_value(&format!("{context}[{i}]"))?);
        }
        Ok(values)
    }

    fn read_until<T: FromFixedBytes + Debug>(
        &mut self,
        context: &str,
        pred: impl Fn(&T) -> bool,
    ) -> Result<Vec<T>>;

    /// Splits the block into chunks of the given size. Panics if the block size
    /// is not a multiple of the chunk size.
    fn with_chunks<F, R, E>(&mut self, chunk_size: usize, context: &str, body: F) -> Result<Vec<R>>
    where
        F: for<'b> FnMut(Self::Sibling<'b>) -> std::result::Result<R, E>,
        E: std::error::Error + Send + Sync + 'static;

    fn split_values<T: FromFixedBytes>(&mut self, context: &str) -> Result<Vec<T>> {
        self.with_chunks(T::SIZE, context, |mut r| r.read_value::<T>("value"))
    }

    fn read_length_delimited_block<'b>(
        &'b mut self,
        context: &'b str,
        item_size: usize,
    ) -> Result<Self::Sibling<'b>> {
        let num_blocks = self.read_value::<u16>(&format!("{context}(length)"))?;
        let total_block_size = usize::from(num_blocks)
            .checked_mul(item_size)
            .expect("Size calculation should not overflow");
        self.read_to_subreader(format!("{context}(contents)"), total_block_size)
    }

    /// Reads a sequence of values, where the first value is a little endian
    /// u16 indicating the number of values to read.
    fn read_length_delimited_records<T: FromFixedBytes>(
        &mut self,
        context: &str,
    ) -> Result<Vec<T>> {
        let num_records = self.read_value::<u16>(&format!("{context}(length)"))?;
        self.read_values::<T>(&format!("{context}(values)"), num_records as usize)
    }

    fn read_u8(&mut self) -> Result<u8> {
        let buf = self.read_exact(1)?;
        Ok(buf[0])
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let buf = self.read_exact(2)?;
        Ok(u16::from_le_bytes(buf.try_into().unwrap()))
    }

    fn read_u24_le(&mut self) -> Result<u32> {
        let buf = self.read_exact(3)?;
        assert!(buf.len() == 3);
        Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], 0]))
    }

    fn read_u32_le(&mut self) -> Result<u32> {
        let buf = self.read_exact(4)?;
        Ok(u32::from_le_bytes(buf.try_into().unwrap()))
    }
}

#[derive(Clone)]
pub struct BufferMemReader<'a, B: 'a> {
    buffer: &'a B,
    start: usize,
    end: usize,
    position: usize,
    context: BlockContext<'a>,
}

impl<'a, B: 'a> Debug for BufferMemReader<'a, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemReader")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("position", &self.position)
            .finish()
    }
}

impl<'a, B: 'a> BufferMemReader<'a, B>
where
    B: Buffer + 'a,
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

    #[track_caller]
    #[allow(unsafe_code)]
    fn err_with_context<E>(&self) -> impl FnOnce(E) -> Error
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        move |err| {
            convert_if_different(err, |err| {
                Error::new(self.context.create_error(self.position, err))
            })
        }
    }

    #[track_caller]
    fn err_with_message<Msg>(&self, message: Msg) -> Error
    where
        Msg: Into<String>,
    {
        Error::new(
            self.context
                .create_error(self.position, OtherError::from_msg(message.into())),
        )
    }

    fn make_sub_reader<NewD>(
        &self,
        start: usize,
        end: usize,
        context: NewD,
    ) -> BufferMemReader<'_, B>
    where
        NewD: Display + Debug + Clone + 'static,
    {
        let new_start = self.start + start;
        let new_end = self.start + end;
        BufferMemReader {
            buffer: self.buffer,
            start: new_start,
            end: new_end,
            position: 0,
            context: self.context.nested(self.start, self.end, context),
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
            context: BlockContext::new_root(buf.size()),
        }
    }
}

impl<'a, B: 'a> MemReader<'a> for BufferMemReader<'a, B>
where
    B: Buffer,
{
    type Sibling<'b>
        = BufferMemReader<'b, B>
    where
        Self: 'b;

    fn tell(&self) -> usize {
        self.position
    }

    fn data_size(&self) -> usize {
        self.end - self.start
    }

    fn seek_to(&mut self, offset: usize) -> Result<()> {
        if self.data_size() < offset {
            return Err(self.err_with_context()(BufferError::NotEnoughData {
                required: offset,
                available: self.data_size(),
            }));
        }

        self.position = offset;
        Ok(())
    }

    fn read_exact(&mut self, len: usize) -> Result<&[u8]> {
        let buffer_data = self.get_slice(len)?;
        self.position += len;
        Ok(buffer_data)
    }

    fn remaining(&self) -> usize {
        self.end - self.start - self.position
    }

    fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    fn read_remainder_slice(&mut self) -> &'a [u8] {
        self.get_whole_slice()
    }

    fn read_until<T: FromFixedBytes + Debug>(
        &mut self,
        context: &str,
        pred: impl Fn(&T) -> bool,
    ) -> Result<Vec<T>> {
        let mut values = Vec::new();
        while !self.is_empty() {
            let mut value_data = self.read_to_subreader(context, T::SIZE)?;
            let value = T::parse(value_data.read_remainder_slice());
            let matches = pred(&value);
            values.push(value);
            if matches {
                return Ok(values);
            }
        }
        Err(self.err_with_message("Got to end before matching value."))
    }

    fn sub_reader_range<'b, R, Ctxt>(
        &'b self,
        context: Ctxt,
        range: R,
    ) -> Result<BufferMemReader<'b, B>>
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
            context.into().into_owned(),
        ))
    }

    fn read_to_subreader<'b, Ctxt>(
        &'b mut self,
        context: Ctxt,
        len: usize,
    ) -> Result<BufferMemReader<'b, B>>
    where
        Ctxt: Into<Cow<'b, str>>,
    {
        self.check_read_length(len)?;
        let old_position = self.position;
        self.position += len;
        let sub_reader = self.make_sub_reader(
            old_position,
            old_position + len,
            context.into().into_owned(),
        );
        Ok(sub_reader)
    }

    /// Splits the block into chunks of the given size. Panics if the block size
    /// is not a multiple of the chunk size.
    fn with_chunks<F, R, E>(
        &mut self,
        chunk_size: usize,
        context: &str,
        mut body: F,
    ) -> Result<Vec<R>>
    where
        F: for<'b> FnMut(BufferMemReader<'b, B>) -> std::result::Result<R, E>,
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
            let sub_reader =
                self.make_sub_reader(self.position, self.position + chunk_size, entry_context);

            chunks.push(body(sub_reader).map_err(self.err_with_context())?);
            self.position += chunk_size;
            index += 1;
        }
        Ok(chunks)
    }
}
