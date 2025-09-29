use crate::fs::err_helpers::io_bail;

/// Provides an upper bound on the data that can be read from a reader. If the
/// data available from the reader exceeds the limit, it will return an
/// [`std::io::ErrorKind::UnexpectedEof`] error.
pub struct LengthLimitedReader<R> {
    inner: R,
    remaining: u64,
}

impl<R> LengthLimitedReader<R> {
    /// Creates a new `LengthLimitedReader` that wraps the given reader
    /// and limits the amount of data that can be read to `max_bytes`.
    ///
    /// # Panics
    ///
    /// Panics if `max_bytes` is zero.
    #[must_use]
    pub fn new(inner: R, max_bytes: u64) -> Self {
        assert!(max_bytes > 0, "max_bytes must be greater than zero");
        Self {
            inner,
            remaining: max_bytes,
        }
    }
}

impl<R> std::io::Read for LengthLimitedReader<R>
where
    R: std::io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read_bytes = self.inner.read(buf)?;
        self.remaining = self
            .remaining
            .saturating_sub(read_bytes.try_into().unwrap());
        if self.remaining == 0 {
            // The underlying reader has provided more data than we expected.
            io_bail!(UnexpectedEof, "Input longer than expected.");
        }
        Ok(read_bytes)
    }
}
