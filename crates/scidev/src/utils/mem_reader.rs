use std::{
    borrow::Cow,
    error::Error as StdError,
    fmt::{Debug, Display},
    ops::Bound,
};

use bytes::BufMut;

use crate::utils::{
    buffer::{FallibleBuffer, FallibleBufferRef},
    convert::convert_if_different,
    errors::{AnyInvalidDataError, BlockContext, InvalidDataError, NoError, OtherError},
};

pub trait ToFixedBytes {
    const SIZE: usize;
    fn to_bytes(&self, dest: &mut [u8]);
}

pub trait FromFixedBytes: Sized {
    const SIZE: usize;
    fn parse<B: bytes::Buf>(bytes: B) -> Self;
}

macro_rules! impl_fixed_bytes_for_num {
    ($($num:ty),*) => {
        $(
            impl ToFixedBytes for $num {
                const SIZE: usize = std::mem::size_of::<$num>();

                fn to_bytes(&self, dest: &mut [u8]) {
                    dest.copy_from_slice(&self.to_le_bytes());
                }
            }

            impl FromFixedBytes for $num {
                const SIZE: usize = std::mem::size_of::<$num>();

                fn parse<B: bytes::Buf>(bytes: B) -> Self {
                    let mut byte_array = [0u8; <Self as FromFixedBytes>::SIZE];
                    (&mut byte_array[..]).put(bytes);
                    Self::from_le_bytes(byte_array)
                }
            }
        )*
    };
}

impl_fixed_bytes_for_num!(i8, i16, i32, i64, i128, isize);
impl_fixed_bytes_for_num!(u8, u16, u32, u64, u128, usize);

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BufferError {
    #[error("Not enough data in buffer. Needed {required}, but only {available} available.")]
    NotEnoughData { required: usize, available: usize },
    #[error("Buffer size is not a multiple of {required}. Had overflow of {overflow} instead.")]
    NotDivisible { required: usize, overflow: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum Error<E> {
    #[error(transparent)]
    BaseError(E),
    #[error(transparent)]
    InvalidData(AnyInvalidDataError),
}

impl<E, BaseErr> From<InvalidDataError<BaseErr>> for Error<E>
where
    E: StdError + Send + Sync + 'static,
    BaseErr: StdError + Send + Sync + 'static,
{
    fn from(err: InvalidDataError<BaseErr>) -> Self {
        Self::InvalidData(err.into())
    }
}

impl<E> From<AnyInvalidDataError> for Error<E>
where
    E: StdError + Send + Sync + 'static,
{
    fn from(err: AnyInvalidDataError) -> Self {
        Self::InvalidData(err)
    }
}

impl<E> Error<E>
where
    E: StdError + Send + Sync + 'static,
{
    pub fn new<Err>(invalid_data: InvalidDataError<Err>) -> Self
    where
        Err: StdError + Send + Sync + 'static,
    {
        Self::InvalidData(invalid_data.into())
    }
}

pub type Result<T, E> = std::result::Result<T, Error<E>>;

pub trait MemReader {
    type Error: StdError + Send + Sync + 'static;

    fn seek_to(&mut self, offset: usize) -> Result<(), Self::Error>;

    #[must_use]
    fn tell(&self) -> usize;

    #[must_use]
    fn data_size(&self) -> usize;

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error>;

    #[must_use]
    fn remaining(&self) -> usize;

    /// Create an `InvalidDataError` with the current context and backtrace.
    fn create_invalid_data_error<Err>(&self, message: Err) -> InvalidDataError<Err>
    where
        Err: StdError + Send + Sync + 'static;

    fn create_invalid_data_error_msg<'a, Msg>(&self, message: Msg) -> AnyInvalidDataError
    where
        Msg: Into<Cow<'a, str>>,
    {
        self.create_invalid_data_error(OtherError::from_msg(message.into().into_owned()))
            .into()
    }

    #[must_use]
    fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    fn read_some<'buf>(&mut self, buf: &'buf mut [u8]) -> Result<&'buf [u8], Self::Error> {
        let remaining = self.remaining();
        let len = std::cmp::min(remaining, buf.len());
        let buf = &mut buf[..len];
        self.read_exact(buf)?;
        Ok(buf)
    }

    fn read_remaining(&mut self) -> std::result::Result<Vec<u8>, Self::Error> {
        let remaining = self.remaining();
        let mut buf = vec![0; remaining];
        self.read_exact(&mut buf).map_err(|e| match e {
            Error::BaseError(err) => err,
            Error::InvalidData(_) => unreachable!(),
        })?;
        Ok(buf)
    }

    fn sub_reader_range<'b, R, Ctxt>(
        &'b self,
        context: Ctxt,
        range: R,
    ) -> Result<impl MemReader<Error = Self::Error> + 'b, Self::Error>
    where
        R: std::ops::RangeBounds<usize>,
        Ctxt: Into<Cow<'b, str>>;

    fn sub_reader_at_offset_length<'b, Ctxt>(
        &'b self,
        context: Ctxt,
        offset: usize,
        length: Option<usize>,
    ) -> Result<impl MemReader<Error = Self::Error> + 'b, Self::Error>
    where
        Ctxt: Into<Cow<'b, str>>,
    {
        self.sub_reader_range(context, offset..offset + length.unwrap_or(self.remaining()))
    }

    fn read_to_subreader<'b, Ctxt>(
        &'b mut self,
        context: Ctxt,
        len: usize,
    ) -> Result<impl MemReader<Error = Self::Error> + 'b, Self::Error>
    where
        Ctxt: Into<Cow<'b, str>>;

    /// Reads a value from the front of the buffer, returning the value and the
    /// remaining buffer.
    // Functions that can be implemented in terms of the above functions.
    fn read_value<T: FromFixedBytes>(&mut self, context: &str) -> Result<T, Self::Error> {
        let mut const_buf = [0u8; 16];
        let mut dyn_buf = Vec::new();

        let buf = if T::SIZE <= const_buf.len() {
            &mut const_buf[..T::SIZE]
        } else {
            dyn_buf.resize(T::SIZE, 0);
            &mut dyn_buf[..]
        };
        let mut subreader = self.read_to_subreader(context, T::SIZE)?;
        subreader.read_exact(&mut buf[..])?;
        let value = T::parse(&buf[..]);
        Ok(value)
    }

    /// Reads N values from the front of the buffer, returning the values and the
    /// remaining buffer.
    fn read_values<T: FromFixedBytes>(
        &mut self,
        context: &str,
        count: usize,
    ) -> Result<Vec<T>, Self::Error> {
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
    ) -> Result<Vec<T>, Self::Error>;

    fn split_values<T: FromFixedBytes>(&mut self, context: &str) -> Result<Vec<T>, Self::Error>;

    fn read_length_delimited_block<'b>(
        &'b mut self,
        context: &'b str,
        item_size: usize,
    ) -> Result<impl MemReader<Error = Self::Error> + 'b, Self::Error> {
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
    ) -> Result<Vec<T>, Self::Error> {
        let num_records = self.read_value::<u16>(&format!("{context}(length)"))?;
        self.read_values::<T>(&format!("{context}(values)"), num_records as usize)
    }

    fn read_u8(&mut self) -> Result<u8, Self::Error> {
        let mut buf = [0u8; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn read_u16_le(&mut self) -> Result<u16, Self::Error> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u24_le(&mut self) -> Result<u32, Self::Error> {
        let mut buf = [0u8; 3];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], 0]))
    }

    fn read_u32_le(&mut self) -> Result<u32, Self::Error> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
}

#[derive(Clone)]
pub struct BufferMemReader<'a, B> {
    buffer: B,
    start: usize,
    end: usize,
    position: usize,
    context: BlockContext<'a>,
}

impl<B> Debug for BufferMemReader<'_, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemReader")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("position", &self.position)
            .finish()
    }
}

impl<B> BufferMemReader<'_, B>
where
    B: FallibleBuffer + Clone,
{
    fn check_read_length(&self, len: usize) -> Result<(), B::Error> {
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
    fn err_with_context<E>(&self) -> impl FnOnce(E) -> Error<B::Error>
    where
        E: StdError + Send + Sync + 'static,
    {
        move |err| {
            convert_if_different(err, |err| {
                Error::new(self.context.create_error(self.position, err))
            })
        }
    }

    #[track_caller]
    fn err_with_message<Msg>(&self, message: Msg) -> Error<B::Error>
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
            buffer: self.buffer.clone(),
            start: new_start,
            end: new_end,
            position: 0,
            context: self.context.nested(self.start, self.end, context),
        }
    }

    pub fn new(buf: B) -> Self {
        let buf_len = buf.size();
        Self {
            buffer: buf,
            start: 0,
            end: buf_len,
            position: 0,
            context: BlockContext::new_root(buf_len),
        }
    }
}

impl<'a, B> BufferMemReader<'_, FallibleBufferRef<'a, B>>
where
    B: FallibleBuffer + Clone,
{
    pub fn from_ref(buf: &'a B) -> Self {
        Self::new(FallibleBufferRef::from(buf))
    }
}

impl<B> MemReader for BufferMemReader<'_, B>
where
    B: FallibleBuffer + Clone,
{
    type Error = B::Error;

    fn tell(&self) -> usize {
        self.position
    }

    fn data_size(&self) -> usize {
        self.end - self.start
    }

    fn seek_to(&mut self, offset: usize) -> Result<(), B::Error> {
        if self.data_size() < offset {
            return Err(self.err_with_context()(BufferError::NotEnoughData {
                required: offset,
                available: self.data_size(),
            }));
        }

        self.position = offset;
        Ok(())
    }

    fn split_values<T: FromFixedBytes>(&mut self, context: &str) -> Result<Vec<T>, Self::Error> {
        if self.remaining() % T::SIZE != 0 {
            return Err(self.err_with_context()(BufferError::NotDivisible {
                required: T::SIZE,
                overflow: self.remaining() % T::SIZE,
            }));
        }
        let mut values = Vec::with_capacity(self.remaining() / T::SIZE);
        while !self.is_empty() {
            let value = self.read_value::<T>(context)?;
            values.push(value);
        }
        Ok(values)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), B::Error> {
        if self.remaining() < buf.len() {
            return Err(self.err_with_context()(BufferError::NotEnoughData {
                required: buf.len(),
                available: self.remaining(),
            }));
        }

        self.buffer
            .read_slice(self.start + self.position, buf)
            .map_err(Error::BaseError)?;

        self.position += buf.len();
        Ok(())
    }

    fn remaining(&self) -> usize {
        self.end - self.start - self.position
    }

    fn read_until<T: FromFixedBytes + Debug>(
        &mut self,
        context: &str,
        pred: impl Fn(&T) -> bool,
    ) -> Result<Vec<T>, B::Error> {
        let mut values = Vec::new();
        while !self.is_empty() {
            let value = self.read_value::<T>(context)?;
            let matches = pred(&value);
            values.push(value);
            if matches {
                return Ok(values);
            }
        }
        Err(self.err_with_message("Got to end before matching value."))
    }

    fn create_invalid_data_error<Err>(&self, message: Err) -> InvalidDataError<Err>
    where
        Err: StdError + Send + Sync + 'static,
    {
        self.context.create_error(self.position, message)
    }

    fn sub_reader_range<'b, R, Ctxt>(
        &'b self,
        context: Ctxt,
        range: R,
    ) -> Result<impl MemReader<Error = Self::Error> + 'b, B::Error>
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
    ) -> Result<impl MemReader<Error = Self::Error> + 'b, B::Error>
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
}

/// An extension trait for reducing the shape of an error that includes [`NoError`].
pub trait NoErrorResultExt<T> {
    type R;
    fn remove_no_error(self) -> Self::R;
}

impl<T> NoErrorResultExt<T> for std::result::Result<T, Error<NoError>> {
    type R = std::result::Result<T, AnyInvalidDataError>;
    fn remove_no_error(self) -> Self::R {
        match self {
            Ok(value) => Ok(value),
            Err(Error::BaseError(err)) => err.absurd(),
            Err(Error::InvalidData(err)) => Err(err),
        }
    }
}

impl<T> NoErrorResultExt<T> for std::result::Result<T, NoError> {
    type R = T;
    fn remove_no_error(self) -> Self::R {
        match self {
            Ok(value) => value,
            Err(err) => err.absurd(),
        }
    }
}

/// A trait for types that can be parsed from a `MemReader`.
pub trait Parse: Sized {
    /// Parses a value from the given `MemReader`.
    ///
    /// This function should leave the reader at the position immediately after
    /// the parsed value.
    fn parse<M: MemReader>(reader: &mut M) -> Result<Self, M::Error>;
}
