use super::DataBlock;

#[derive(Debug, Clone)]
pub struct SubRange<D> {
    data_source: D,
    offset: u64,
    size: u64,
}

impl<D> SubRange<D>
where
    D: DataBlock,
{
    pub fn new(mut data_source: D, offset: u64, size: u64) -> super::Result<Self> {
        assert!(offset + size <= data_source.size()?);
        Ok(Self {
            data_source,
            offset,
            size,
        })
    }
}

impl<D> DataBlock for SubRange<D>
where
    D: DataBlock,
{
    fn size(&mut self) -> super::Result<u64> {
        Ok(self.size)
    }
}

impl<D> super::ReadBlock for SubRange<D>
where
    D: super::ReadBlock,
{
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> super::Result<()> {
        assert!(offset + buf.len() as u64 <= self.size);
        self.data_source.read_at(self.offset + offset, buf)
    }
}

impl<D> super::WriteBlock for SubRange<D>
where
    D: super::WriteBlock,
{
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> super::Result<()> {
        assert!(offset + buf.len() as u64 <= self.size);
        self.data_source.write_at(self.offset + offset, buf)
    }
}
