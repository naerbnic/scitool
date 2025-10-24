use std::io;

use futures::{AsyncWrite, AsyncWriteExt as _};

pub(super) trait BitWriter {
    /// Write a single bit to the output.
    fn write_bit(&mut self, bit: bool) -> impl Future<Output = io::Result<()>>;

    /// Write a full byte to the output.
    fn write_u8(&mut self, byte: u8) -> impl Future<Output = io::Result<()>>;

    /// Writes `count` lower bits of 'bits' to the output.
    fn write_bits(&mut self, count: u8, bits: u64) -> impl Future<Output = io::Result<()>>;

    /// Returns the number of bits that need to be written to reach the next byte boundary.
    fn bits_until_byte_aligned(&self) -> u8;

    fn finish(self) -> impl Future<Output = io::Result<()>>
    where
        Self: Sized;
}

pub(super) struct LittleEndianWriter<W>
where
    W: AsyncWrite + Unpin,
{
    output: W,
    curr_byte: u8,
    bits_filled: u8,
    finished: bool,
}

impl<W> LittleEndianWriter<W>
where
    W: AsyncWrite + Unpin,
{
    pub(super) fn new(output: W) -> Self {
        LittleEndianWriter {
            output,
            curr_byte: 0,
            bits_filled: 0,
            finished: false,
        }
    }
}

impl<W> BitWriter for LittleEndianWriter<W>
where
    W: AsyncWrite + Unpin,
{
    async fn write_bit(&mut self, bit: bool) -> io::Result<()> {
        if bit {
            self.curr_byte |= 1 << self.bits_filled;
        }
        self.bits_filled += 1;
        if self.bits_filled == 8 {
            self.output.write_all(&[self.curr_byte]).await?;
            self.curr_byte = 0;
            self.bits_filled = 0;
        }
        Ok(())
    }

    async fn write_u8(&mut self, byte: u8) -> io::Result<()> {
        self.write_bits(8, u64::from(byte)).await?;
        Ok(())
    }

    async fn write_bits(&mut self, count: u8, bits: u64) -> io::Result<()> {
        assert!(count <= 64);
        for i in 0..count {
            let bit = (bits >> i) & 1 != 0;
            self.write_bit(bit).await?;
        }
        Ok(())
    }

    fn bits_until_byte_aligned(&self) -> u8 {
        if self.bits_filled == 0 {
            0
        } else {
            8 - self.bits_filled
        }
    }

    async fn finish(mut self) -> io::Result<()>
    where
        Self: Sized,
    {
        if self.bits_filled > 0 {
            self.output.write_all(&[self.curr_byte]).await?;
            self.curr_byte = 0;
            self.bits_filled = 0;
        }
        self.output.flush().await?;
        self.finished = true;
        Ok(())
    }
}

impl<W> Drop for LittleEndianWriter<W>
where
    W: AsyncWrite + Unpin,
{
    fn drop(&mut self) {
        // Dropped without finishing. Attempt to flush remaining bits, but
        // ignore any errors.
        if self.bits_filled > 0 {
            drop(self.output.write_all(&[self.curr_byte]));
        }
    }
}
