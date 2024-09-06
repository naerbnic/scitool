use std::io;

use super::{DataSource, DataTarget};

pub struct Inner<R> {
    reader: R,
    position: Option<u64>,
}

impl<R> Inner<R>
where
    R: io::Seek,
{
    fn get_curr_pos(&mut self) -> io::Result<u64> {
        Ok(match self.position {
            Some(pos) => pos,
            None => self.raw_seek(io::SeekFrom::Current(0))?,
        })
    }

    fn raw_seek(&mut self, seek_from: io::SeekFrom) -> io::Result<u64> {
        let new_pos = self
            .reader
            .seek(seek_from)
            .inspect_err(|_| self.position = None)?;
        self.position = Some(new_pos);
        Ok(new_pos)
    }

    fn seek_to_offset(&mut self, offset: u64) -> io::Result<u64> {
        let old_pos = self.get_curr_pos()?;
        if old_pos == offset {
            return Ok(offset);
        }

        // We add some logic to regularize the behavior of seek
        // when seeking beyond the end of a file.
        let new_pos = self.raw_seek(io::SeekFrom::Start(offset))?;
        if new_pos != offset {
            return Err(io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Seek target is past the end of the file.",
            ));
        }

        Ok(offset)
    }

    pub fn seek(&mut self, seek_from: io::SeekFrom) -> io::Result<u64> {
        let new_pos = match seek_from {
            io::SeekFrom::Start(target_pos) => self.seek_to_offset(target_pos)?,
            io::SeekFrom::Current(curr) => {
                let curr_pos = self.get_curr_pos()?;
                let target_pos = curr_pos.checked_add_signed(curr).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Seeking would result in a negative position",
                    )
                })?;
                self.seek_to_offset(target_pos)?
            }
            io::SeekFrom::End(before_end_pos) => {
                if before_end_pos > 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Seeking past the end of the file is not supported",
                    ));
                }

                // It's guaranteed that if the before_end_pos ends up before
                // byte 0, the seek will fail, so a raw seek is safe here.
                self.raw_seek(io::SeekFrom::End(before_end_pos))?
            }
        };
        Ok(new_pos)
    }
}

impl<R> Inner<R>
where
    R: io::Read,
{
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let old_pos = self.position;
        self.reader
            .read_exact(buf)
            .inspect_err(|_| self.position = None)?;
        if let Some(old_pos) = old_pos {
            self.position = Some(old_pos + buf.len() as u64);
        }
        Ok(())
    }
}

impl<R> Inner<R>
where
    R: io::Write,
{
    pub fn write(&mut self, buf: &[u8]) -> io::Result<()> {
        let old_pos = self.position;
        self.reader
            .write_all(buf)
            .inspect_err(|_| self.position = None)?;
        if let Some(old_pos) = old_pos {
            self.position = Some(old_pos + buf.len() as u64);
        };
        Ok(())
    }
}

impl<R> Inner<R>
where
    R: io::Write + io::Seek,
{
    pub fn ensure_size(&mut self, size: u64) -> io::Result<()> {
        let curr_size = self.seek(io::SeekFrom::End(0))?;
        if curr_size < size {
            const EMPTY_BUFFER: &[u8] = &[0u8; 4096];
            let mut remaining = size - curr_size;
            while remaining > 0 {
                let write_size = remaining
                    .min(EMPTY_BUFFER.len() as u64)
                    .try_into()
                    .expect("Size is always less than 4096");
                self.write(&EMPTY_BUFFER[..write_size])?;
                remaining -= write_size as u64;
            }
        }
        Ok(())
    }
}

pub struct IoSource<R>(Inner<R>);

impl<R> IoSource<R> {
    pub fn new(reader: R) -> Self {
        Self(Inner {
            reader,
            position: None,
        })
    }
}

impl<R> DataSource for IoSource<R>
where
    R: io::Read + io::Seek,
{
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), super::Error> {
        let source = &mut self.0;
        if Some(offset) != source.position {
            let new_pos = source.seek(io::SeekFrom::Start(offset))?;
            if new_pos != offset {
                return Err(super::Error::from(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Failed to seek to the requested position",
                )));
            }
            source.position = Some(offset);
        }
        source.reader.read_exact(buf).inspect_err(|_| {
            // The position afterwards is undefined on an error.
            source.position = None;
        })?;
        source.position = Some(offset + buf.len() as u64);

        Ok(())
    }

    fn size(&mut self) -> Result<u64, super::Error> {
        Ok(self.0.seek(io::SeekFrom::End(0))?)
    }
}

impl<R> DataTarget for IoSource<R>
where
    R: io::Write + io::Seek,
{
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<(), super::Error> {
        let source = &mut self.0;
        source.ensure_size(offset)?;
        source.seek(io::SeekFrom::Start(offset))?;
        source.write(buf)?;
        Ok(())
    }
}
