use std::{
    fmt::Debug,
    io,
    sync::{Arc, Mutex},
};

use crate::utils::{block::core::RangeStreamBase, range::BoundedRange};

pub(super) struct BorrowedReader<'a, R> {
    reader: &'a Mutex<R>,
    position: u64,
    remaining_length: u64,
}

impl<R> io::Read for BorrowedReader<'_, R>
where
    R: io::Read + io::Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.remaining_length == 0 {
            return Ok(0);
        }
        let to_read = std::cmp::min(buf.len().try_into().unwrap(), self.remaining_length)
            .try_into()
            .unwrap();
        let mut reader = self.reader.lock().unwrap();
        reader.seek(io::SeekFrom::Start(self.position))?;
        let read_bytes = reader.read(&mut buf[..to_read])?;
        self.position += read_bytes as u64;
        self.remaining_length -= read_bytes as u64;
        Ok(read_bytes)
    }
}

pub(super) struct ReadSeekImpl<R>(Arc<Mutex<R>>);

impl<R> ReadSeekImpl<R>
where
    R: io::Read + io::Seek,
{
    pub(super) fn new(reader: R) -> Self {
        Self(Arc::new(Mutex::new(reader)))
    }
}

impl<R> RangeStreamBase for ReadSeekImpl<R>
where
    R: io::Read + io::Seek,
{
    type Reader<'a>
        = BorrowedReader<'a, R>
    where
        Self: 'a;
    fn open_range_reader(&self, range: BoundedRange<u64>) -> io::Result<Self::Reader<'_>> {
        let reader = &*self.0;
        Ok(BorrowedReader {
            reader,
            position: range.start(),
            remaining_length: range.size(),
        })
    }
}

impl<R> Debug for ReadSeekImpl<R>
where
    R: io::Read + io::Seek,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadSeekImpl").finish()
    }
}
