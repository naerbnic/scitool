use crate::utils::buffer::{Buffer, BufferError, BufferExt};

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(#[from] BufferError);

pub type Result<T> = std::result::Result<T, Error>;

pub trait MemReader {
    type Buffer;
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()>;
    fn seek_to(&mut self, offset: usize) -> Result<()>;
    fn tell(&self) -> usize;
    fn data_size(&self) -> usize;
    fn into_rest(self) -> Self::Buffer;

    fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let mut buf = [0; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u24_le(&mut self) -> Result<u32> {
        let mut buf = [0; 3];
        self.read_exact(&mut buf[0..3])?;
        Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], 0]))
    }

    fn read_u32_le(&mut self) -> Result<u32> {
        let mut buf = [0; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
}

pub struct SliceMemReader<B> {
    data: B,
    position: usize,
}

impl<B> SliceMemReader<B> {
    pub fn new(buf: B) -> Self {
        Self {
            data: buf,
            position: 0,
        }
    }
}

impl<B> MemReader for SliceMemReader<B>
where
    B: Buffer,
{
    type Buffer = B;
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        let data = self
            .data
            .clone()
            .sub_buffer_from_range(self.position, self.position + buf.len())?;
        buf.copy_from_slice(data.as_slice());
        self.position += buf.len();
        Ok(())
    }

    fn seek_to(&mut self, offset: usize) -> Result<()> {
        if offset > self.data.size() {
            return Err(Error(BufferError::NotEnoughData {
                required: offset,
                available: self.data.size(),
            }));
        }
        self.position = offset;
        Ok(())
    }

    fn tell(&self) -> usize {
        self.position
    }

    fn data_size(&self) -> usize {
        self.data.size()
    }

    fn into_rest(self) -> B {
        let Self { data, position } = self;
        data.sub_buffer(position..).unwrap()
    }
}
