use std::{io, sync::Mutex};

use super::DataSource;

pub struct Inner<R> {
    reader: R,
    position: Option<u64>,
    size: Option<u64>,
}

pub struct IoSource<R>(Mutex<Inner<R>>);

impl<R> IoSource<R> {
    pub fn new(reader: R) -> Self {
        Self(Mutex::new(Inner {
            reader,
            position: None,
            size: None,
        }))
    }
}

impl<R> DataSource for IoSource<R>
where
    R: io::Read + io::Seek,
{
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), super::Error> {
        let mut source = self.0.lock().unwrap();
        let range_end = offset + buf.len() as u64;
        if let Some(size) = source.size {
            if range_end > size {
                return Err(super::Error::from(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Attempted to read past the end of the data source",
                )));
            }
        }
        if Some(offset) != source.position {
            let new_pos = source.reader.seek(io::SeekFrom::Start(offset))?;
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

    fn size(&self) -> Result<u64, super::Error> {
        let mut source = self.0.lock().unwrap();
        match source.size {
            Some(size) => Ok(size),
            None => {
                let size = source.reader.seek(io::SeekFrom::End(0))?;
                source.size = Some(size);
                source.position = Some(size);
                Ok(size)
            }
        }
    }
}
