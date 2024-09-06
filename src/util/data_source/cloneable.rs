use std::sync::{Arc, Mutex};

use super::{BoundedDataSource, BoundedDataTarget, DataSource, DataTarget};

pub struct Cloneable<D> {
    data_source: Arc<Mutex<D>>,
}

impl<D> Cloneable<D> {
    pub fn new(data_source: D) -> Cloneable<D> {
        Cloneable {
            data_source: Arc::new(Mutex::new(data_source)),
        }
    }
}

impl<D> Clone for Cloneable<D> {
    fn clone(&self) -> Self {
        Cloneable {
            data_source: self.data_source.clone(),
        }
    }
}

impl<D> DataSource for Cloneable<D>
where
    D: DataSource,
{
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), super::Error> {
        self.data_source.lock().unwrap().read_at(offset, buf)
    }
}

impl<D> BoundedDataSource for Cloneable<D>
where
    D: BoundedDataSource,
{
    fn size(&mut self) -> Result<u64, super::Error> {
        self.data_source.lock().unwrap().size()
    }
}

impl<D> DataTarget for Cloneable<D>
where
    D: DataTarget,
{
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<(), super::Error> {
        self.data_source.lock().unwrap().write_at(offset, buf)
    }
}

impl<D> BoundedDataTarget for Cloneable<D>
where
    D: BoundedDataTarget,
{
    fn size(&mut self) -> Result<u64, super::Error> {
        self.data_source.lock().unwrap().size()
    }
}
