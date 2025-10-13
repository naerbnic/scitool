use std::io;

use futures::{AsyncRead, AsyncReadExt as _};

pub(super) trait BitReader {
    fn read_bit(&mut self) -> impl Future<Output = io::Result<bool>>;
    fn read_bits(&mut self, count: u32) -> impl Future<Output = io::Result<u64>>;
    fn read_u8(&mut self) -> impl Future<Output = io::Result<u8>>;
}

pub(super) struct LittleEndianReader<R> {
    reader: R,
    curr_byte: u8,
    bits_left: u8,
}

impl<R> LittleEndianReader<R>
where
    R: AsyncRead + Unpin,
{
    pub(super) fn new(reader: R) -> Self {
        Self {
            reader,
            curr_byte: 0,
            bits_left: 0,
        }
    }
}

impl<R> BitReader for LittleEndianReader<R>
where
    R: AsyncRead + Unpin,
{
    async fn read_bit(&mut self) -> io::Result<bool> {
        if self.bits_left == 0 {
            let mut byte = [0u8; 1];
            self.reader.read_exact(&mut byte).await?;
            self.curr_byte = byte[0];
            self.bits_left = 8;
        }
        let next_bit = self.curr_byte & 1 != 0;
        self.bits_left -= 1;
        self.curr_byte >>= 1;
        Ok(next_bit)
    }

    async fn read_bits(&mut self, count: u32) -> io::Result<u64> {
        assert!(count <= 64);
        let mut value = 0u64;
        for i in 0..count {
            if self.read_bit().await? {
                value |= 1 << i;
            }
        }
        Ok(value)
    }
    async fn read_u8(&mut self) -> io::Result<u8> {
        self.read_bits(8).await.map(|b| u8::try_from(b).unwrap())
    }
}
