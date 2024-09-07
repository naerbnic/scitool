use std::sync::{Arc, Mutex};

use super::{DataBlock, ReadBlock, Result};

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

impl<D> ReadBlock for Cloneable<D>
where
    D: ReadBlock,
{
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        self.data_source.lock().unwrap().read_at(offset, buf)
    }
}

impl<D> DataBlock for Cloneable<D>
where
    D: DataBlock,
{
    fn size(&mut self) -> Result<u64> {
        self.data_source.lock().unwrap().size()
    }
}
