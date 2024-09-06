use super::{BoundedDataSource, BoundedDataTarget, DataSource, DataTarget, Error};

pub struct Bounded<D> {
    data_source: D,
    size: u64,
}

impl<D> DataSource for Bounded<D>
where
    D: DataSource,
{
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Error> {
        if offset + buf.len() as u64 > self.size {
            return Err(Error::from(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Attempted to read past the end of the data source",
            )));
        }
        self.data_source.read_at(offset, buf)
    }
}

impl<D> BoundedDataSource for Bounded<D>
where
    D: DataSource,
{
    fn size(&mut self) -> Result<u64, Error> {
        Ok(self.size)
    }
}

impl<D> DataTarget for Bounded<D>
where
    D: DataTarget,
{
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<(), Error> {
        if offset + buf.len() as u64 > self.size {
            return Err(Error::from(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Attempted to write past the end of the data source",
            )));
        }
        self.data_source.write_at(offset, buf)
    }
}

impl<D> BoundedDataTarget for Bounded<D>
where
    D: DataTarget,
{
    fn size(&mut self) -> Result<u64, Error> {
        Ok(self.size)
    }
}
