use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    task::{Context, Poll, ready},
};

use futures::FutureExt;
use scidev_errors::{ResultExt, bail, diag, ensure};
use tokio::io::{AsyncRead, ReadBuf};

use crate::utils::{
    block::{
        MemBlock,
        core::{Block, BlockBase, BoxedAsyncRead, BoxedRead, OpenBaseResult},
    },
    range::BoundedRange,
};

#[derive(Debug)]
pub(super) struct SequenceBlockImpl {
    blocks: Vec<Block>,
}

impl SequenceBlockImpl {
    pub(super) fn new(blocks: impl IntoIterator<Item = Block>) -> Self {
        Self {
            blocks: blocks.into_iter().collect(),
        }
    }

    pub(super) fn size(&self) -> u64 {
        self.blocks.iter().map(Block::len).sum()
    }
}

macro_rules! try_ready {
    ($ex:expr) => {
        match ready!($ex) {
            Ok(val) => val,
            Err(err) => return Poll::Ready(Err(err.into())),
        }
    };
}

impl BlockBase for SequenceBlockImpl {
    fn open_mem(&self, range: BoundedRange<u64>) -> OpenBaseResult<MemBlock> {
        let mut data = Vec::new();
        let mut remaining_range = range;
        let mut iter = self.blocks.iter();
        while remaining_range.size() > 0
            && let Some(curr_block) = iter.next()
        {
            if let Some(curr_range) = remaining_range.intersect(0..curr_block.len()) {
                data.push(
                    curr_block
                        .open_mem(curr_range)
                        .raise_err_with(diag!(|| "Could not open block in sequence"))?,
                );
            }
            remaining_range = remaining_range.shift_down_by(curr_block.len());
        }
        Ok(MemBlock::concat_blocks(data))
    }

    async fn open_mem_async(&self, range: BoundedRange<u64>) -> OpenBaseResult<MemBlock> {
        let mut data = Vec::new();
        let mut remaining_range = range;
        let mut iter = self.blocks.iter();
        while remaining_range.size() > 0
            && let Some(curr_block) = iter.next()
        {
            if let Some(curr_range) = remaining_range.intersect(0..curr_block.len()) {
                data.push(
                    curr_block
                        .open_mem(curr_range)
                        .raise_err_with(diag!(|| "Could not open block in sequence"))?,
                );
            }
            remaining_range = remaining_range.shift_down_by(curr_block.len());
        }
        Ok(MemBlock::concat_blocks(data))
    }

    fn open_reader(
        &self,
        range: BoundedRange<u64>,
    ) -> OpenBaseResult<impl io::Read + Send + 'static> {
        struct SequenceReader {
            remaining_size: u64,
            remaining_blocks: VecDeque<Block>,
            current_reader: Option<BoxedRead>,
        }

        impl io::Read for SequenceReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.remaining_size == 0 {
                    self.current_reader = None;
                    self.remaining_blocks = VecDeque::new();
                    return Ok(0);
                }
                loop {
                    let reader = if let Some(r) = &mut self.current_reader {
                        r
                    } else {
                        let Some(next_block) = self.remaining_blocks.pop_front() else {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "no more blocks to read",
                            ));
                        };
                        // FIXME: This should be another error than OpenError, as OpenError is
                        // an unactionable error.
                        self.current_reader = Some(
                            next_block
                                .open_reader(BoundedRange::from_size(next_block.len()))
                                .map_err(io::Error::other)?,
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
            return Ok(SequenceReader {
                remaining_size: 0,
                remaining_blocks: VecDeque::new(),
                current_reader: None,
            });
        }

        let mut remaining_range = range;
        let mut blocks = &self.blocks[..];

        let first_block = loop {
            ensure!(
                remaining_range.size() > 0,
                "Range extends beyond end of sequence block: {remaining_range:?}"
            );
            let Some((first_block, rest)) = blocks.split_first() else {
                bail!("Range extends beyond end of sequence block: {remaining_range:?}");
            };
            blocks = rest;
            if remaining_range.start() < first_block.len() {
                break first_block;
            }
            remaining_range = remaining_range.shift_down_by(first_block.len());
        };

        let initial_reader = first_block
            .open_reader(remaining_range.intersect(0..first_block.len()).unwrap())
            .raise_err_with(diag!(|| "Failed to open initial reader"))?;

        Ok(SequenceReader {
            remaining_size: remaining_range.size(),
            remaining_blocks: blocks.iter().cloned().collect(),
            current_reader: Some(initial_reader),
        })
    }

    async fn open_async_reader(
        &self,
        range: BoundedRange<u64>,
    ) -> OpenBaseResult<impl AsyncRead + Send + 'static> {
        enum ReaderState {
            GettingReader(
                Pin<Box<dyn Future<Output = io::Result<BoxedAsyncRead>> + Send + 'static>>,
            ),
            Reading(Pin<BoxedAsyncRead>),
            NotReading,
        }
        struct SequenceReader {
            remaining_size: u64,
            remaining_blocks: VecDeque<Block>,
            current_state: ReaderState,
        }

        impl AsyncRead for SequenceReader {
            fn poll_read(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &mut ReadBuf<'_>,
            ) -> Poll<io::Result<()>> {
                loop {
                    if self.remaining_size == 0 {
                        self.current_state = ReaderState::NotReading;
                        self.remaining_blocks = VecDeque::new();
                        return Poll::Ready(Ok(()));
                    }
                    match &mut self.current_state {
                        ReaderState::GettingReader(future) => {
                            self.current_state =
                                ReaderState::Reading(Box::pin(try_ready!(future.poll_unpin(cx))));
                        }
                        ReaderState::Reading(async_read) => {
                            let init_remaining = buf.remaining();
                            try_ready!(async_read.as_mut().poll_read(cx, buf));
                            let bytes_read = init_remaining - buf.remaining();
                            if bytes_read == 0 {
                                self.current_state = ReaderState::NotReading;
                                continue;
                            }
                            self.remaining_size -= u64::try_from(bytes_read).unwrap();
                            return Poll::Ready(Ok(()));
                        }
                        ReaderState::NotReading => {
                            let Some(next_block) = self.remaining_blocks.pop_front() else {
                                return Poll::Ready(Err(io::Error::new(
                                    io::ErrorKind::UnexpectedEof,
                                    "no more blocks to read",
                                )));
                            };

                            self.current_state = ReaderState::GettingReader(Box::pin(async move {
                                next_block
                                    .open_async_reader(BoundedRange::from_size(next_block.len()))
                                    .await
                                    .map_err(io::Error::other)
                            }));
                        }
                    }
                }
            }
        }

        if range.size() == 0 {
            return Ok(SequenceReader {
                remaining_size: 0,
                remaining_blocks: VecDeque::new(),
                current_state: ReaderState::NotReading,
            });
        }

        let mut remaining_range = range;
        let mut blocks = &self.blocks[..];

        let first_block = loop {
            ensure!(
                remaining_range.size() > 0,
                "Range extends beyond end of sequence block: {remaining_range:?}"
            );
            let Some((first_block, rest)) = blocks.split_first() else {
                bail!("Range extends beyond end of sequence block: {remaining_range:?}");
            };
            blocks = rest;
            if remaining_range.start() < first_block.len() {
                break first_block;
            }
            remaining_range = remaining_range.shift_down_by(first_block.len());
        };

        let initial_reader = first_block
            .open_async_reader(remaining_range.intersect(0..first_block.len()).unwrap())
            .await
            .raise_err_with(diag!(|| "Failed to open initial reader"))?;

        Ok(SequenceReader {
            remaining_size: remaining_range.size(),
            remaining_blocks: blocks.iter().cloned().collect(),
            current_state: ReaderState::Reading(Box::into_pin(
                Box::new(initial_reader) as BoxedAsyncRead
            )),
        })
    }
}
