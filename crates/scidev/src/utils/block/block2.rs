use std::{
    io::{self, Read as _, Seek as _},
    ops::RangeBounds,
    sync::{Arc, Mutex},
};

use crate::utils::{
    block::MemBlock,
    buffer::BufferExt as _,
    range::{BoundedRange, Range},
};

pub type ReadScopeFn<'a> = dyn FnMut(&mut dyn io::Read) -> io::Result<()> + Send + 'a;

trait BlockBase {
    // Features we want to support:
    //
    // - A block that needs to be read from disk at a known offset/length.
    // - A block that is generated on demand, possibly from other blocks.
    // - Splitting of blocks into sub-blocks, without the data having to be
    //   loaded first.
    // - Caching of loaded blocks.
    // - Sharing data with other blocks (e.g., via reference counting).
    //
    // Constraints:
    //
    // - Data in a block is considered stable, so opening it multiple times
    //   should yield the same data (or cause an error).
    //
    // This implies we have to have the following capabilities:
    //
    // - Open a full block contents (size unknown).
    // - Open a sub-range of a block (when size known).
    // - Read the contents of a block within a range (when size known).
    // - Read all of the contents of a block (size unknown) while being able to
    //   stop before the end.

    // Open as loaded data, possibly shared.
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock>;

    /// Open as borrowed reader.
    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>>;
}

trait MemBlockBase {
    fn load_mem_block(&self) -> io::Result<&MemBlock>;
}

trait RangeStreamBase {
    type Reader<'a>: io::Read + 'a
    where
        Self: 'a;
    fn open_range_reader(&self, range: BoundedRange<u64>) -> io::Result<Self::Reader<'_>>;
}

trait FullStreamBase {
    type Reader<'a>: io::Read + 'a
    where
        Self: 'a;
    fn open_full_reader(&self) -> io::Result<Self::Reader<'_>>;
}

struct MemBlockWrap<T>(T);

impl<T> BlockBase for MemBlockWrap<T>
where
    T: MemBlockBase,
{
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock> {
        let mem_block = self.0.load_mem_block()?;
        let mem_block = mem_block
            .sub_buffer(range.cast_to::<usize>())
            .map_err(io::Error::other)?;
        Ok(mem_block.clone())
    }

    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        let mem_block = self.0.load_mem_block()?;
        let mem_block = mem_block
            .sub_buffer(range.cast_to::<usize>())
            .map_err(io::Error::other)?;
        Ok(Box::new(io::Cursor::new(mem_block)))
    }
}

struct RangeStreamBaseWrap<T>(T);

impl<T> BlockBase for RangeStreamBaseWrap<T>
where
    T: RangeStreamBase,
{
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock> {
        let mut data = Vec::new();
        self.0.open_range_reader(range)?.read_to_end(&mut data)?;
        Ok(MemBlock::from_vec(data))
    }

    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        let reader = self.0.open_range_reader(range)?;
        Ok(Box::new(reader))
    }
}

struct FullStreamBaseWrap<T>(T);

impl<T> BlockBase for FullStreamBaseWrap<T>
where
    T: FullStreamBase,
{
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock> {
        let mut data = Vec::new();
        self.open_reader(range)?.read_to_end(&mut data)?;
        Ok(MemBlock::from_vec(data))
    }

    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        let mut reader = self.0.open_full_reader()?;
        let temp_buffer = &mut [0u8; 8192];
        let mut data_remaining = range.start();
        while data_remaining > 0 {
            let to_read = std::cmp::min(data_remaining, temp_buffer.len() as u64);
            let read_bytes = reader.read(&mut temp_buffer[..to_read.try_into().unwrap()])?;
            if read_bytes == 0 {
                break;
            }
            data_remaining -= read_bytes as u64;
        }
        Ok(Box::new(reader.take(range.size())))
    }
}

pub trait RefFactory {
    type Output<'a>
    where
        Self: 'a;

    fn create_new(&self) -> io::Result<Self::Output<'_>>;
}

impl<F, T> RefFactory for F
where
    F: Fn() -> io::Result<T>,
{
    type Output<'a>
        = T
    where
        Self: 'a;

    fn create_new(&self) -> io::Result<Self::Output<'_>> {
        self()
    }
}

pub struct SequenceBlockImpl {
    blocks: Vec<Block>,
}

impl BlockBase for SequenceBlockImpl {
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock> {
        let mut data = Vec::new();
        let mut remaining_range = range;
        let mut iter = self.blocks.iter();
        while remaining_range.size() > 0
            && let Some(curr_block) = iter.next()
        {
            if let Some(curr_range) = remaining_range.intersect(0..curr_block.len()) {
                data.push(curr_block.open_mem(curr_range)?);
            }
            remaining_range = remaining_range.shift_down_by(curr_block.len());
        }
        Ok(MemBlock::concat_blocks(data))
    }

    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        struct SequenceReader<'a> {
            remaining_size: u64,
            remaining_blocks: &'a [Block],
            current_reader: Option<Box<dyn io::Read + 'a>>,
        }

        impl io::Read for SequenceReader<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.remaining_size == 0 {
                    self.current_reader = None;
                    self.remaining_blocks = &[];
                    return Ok(0);
                }
                loop {
                    let reader = if let Some(r) = &mut self.current_reader {
                        r
                    } else {
                        let Some((next_block, remaining)) = self.remaining_blocks.split_first()
                        else {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "no more blocks to read",
                            ));
                        };
                        self.remaining_blocks = remaining;
                        self.current_reader = Some(
                            next_block.open_reader(BoundedRange::from_size(next_block.len()))?,
                        );
                        self.current_reader.as_mut().unwrap()
                    };
                    let to_read = std::cmp::min(buf.len().try_into().unwrap(), self.remaining_size)
                        .try_into()
                        .unwrap();
                    let read_bytes = reader.read(&mut buf[..to_read])?;
                    if read_bytes != 0 {
                        self.remaining_size -= read_bytes as u64;
                        return Ok(read_bytes);
                    }
                    self.current_reader = None;
                }
            }
        }

        if range.size() == 0 {
            return Ok(Box::new(io::empty()));
        }

        let mut remaining_range = range;
        let mut blocks = &self.blocks[..];

        let first_block = loop {
            if remaining_range.size() == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "range extends beyond end of sequence block",
                ));
            }
            let Some((first_block, rest)) = blocks.split_first() else {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "range extends beyond end of sequence block",
                ));
            };
            blocks = rest;
            if remaining_range.start() < first_block.len() {
                break first_block;
            }
            remaining_range = remaining_range.shift_down_by(first_block.len());
        };

        let initial_reader =
            first_block.open_reader(remaining_range.intersect(0..first_block.len()).unwrap())?;

        Ok(Box::new(SequenceReader {
            remaining_size: remaining_range.size(),
            remaining_blocks: blocks,
            current_reader: Some(initial_reader),
        }))
    }
}

pub struct Block {
    source: Arc<dyn BlockBase + Send + Sync>,
    range: BoundedRange<u64>,
}

impl Block {
    #[must_use]
    pub fn from_mem_block(mem_block: MemBlock) -> Self {
        struct ContainedMemBlock(MemBlock);

        impl MemBlockBase for ContainedMemBlock {
            fn load_mem_block(&self) -> io::Result<&MemBlock> {
                Ok(&self.0)
            }
        }

        let len = mem_block.len() as u64;

        Block {
            source: Arc::new(MemBlockWrap(ContainedMemBlock(mem_block))),
            range: BoundedRange::from_size(len),
        }
    }

    pub fn from_read_seek_factory<F>(reader_factory: F) -> io::Result<Self>
    where
        F: RefFactory + Send + Sync + 'static,
        for<'a> F::Output<'a>: io::Read + io::Seek,
    {
        struct ReaderBlockSource<F>(F);

        impl<F> RangeStreamBase for ReaderBlockSource<F>
        where
            F: RefFactory,
            for<'a> F::Output<'a>: io::Read + io::Seek,
        {
            type Reader<'a>
                = io::Take<F::Output<'a>>
            where
                Self: 'a;
            fn open_range_reader(&self, range: BoundedRange<u64>) -> io::Result<Self::Reader<'_>> {
                let mut reader = self.0.create_new()?;
                reader.seek(io::SeekFrom::Start(range.start()))?;
                Ok(reader.take(range.size()))
            }
        }

        let size = {
            let mut reader = reader_factory.create_new()?;
            reader.seek(io::SeekFrom::End(0))?
        };

        Ok(Block {
            source: Arc::new(RangeStreamBaseWrap(ReaderBlockSource(reader_factory))),
            range: BoundedRange::from_size(size),
        })
    }

    pub fn from_read_seek<R>(mut reader: R) -> io::Result<Self>
    where
        R: io::Read + io::Seek + Send + 'static,
    {
        struct BorrowedReader<'a, R> {
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

        struct ReaderBlockSource<R>(Arc<Mutex<R>>);

        impl<R> RangeStreamBase for ReaderBlockSource<R>
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
        let size = reader.seek(io::SeekFrom::End(0))?;
        reader.seek(io::SeekFrom::Start(0))?;

        Ok(Block {
            source: Arc::new(RangeStreamBaseWrap(ReaderBlockSource(Arc::new(
                Mutex::new(reader),
            )))),
            range: BoundedRange::from_size(size),
        })
    }

    pub fn from_read_size<F>(reader_factory: F, size: u64) -> Self
    where
        F: RefFactory + Send + Sync + 'static,
        for<'a> F::Output<'a>: io::Read,
    {
        struct ReaderBlockSource<F>(F);

        impl<F> FullStreamBase for ReaderBlockSource<F>
        where
            F: RefFactory,
            for<'a> F::Output<'a>: io::Read,
        {
            type Reader<'a>
                = F::Output<'a>
            where
                Self: 'a;
            fn open_full_reader(&self) -> io::Result<Self::Reader<'_>> {
                self.0.create_new()
            }
        }

        Block {
            source: Arc::new(FullStreamBaseWrap(ReaderBlockSource(reader_factory))),
            range: BoundedRange::from_size(size),
        }
    }

    pub fn open_mem<R>(&self, range: R) -> io::Result<MemBlock>
    where
        R: RangeBounds<u64>,
    {
        let range = Range::from_range(range);
        self.source.open_mem(self.range.new_relative(range))
    }

    pub fn open_reader<'a, R>(&'a self, range: R) -> io::Result<Box<dyn io::Read + 'a>>
    where
        R: RangeBounds<u64>,
    {
        let range = Range::from_range(range);
        self.source.open_reader(self.range.new_relative(range))
    }

    #[must_use]
    pub fn len(&self) -> u64 {
        self.range.size()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
