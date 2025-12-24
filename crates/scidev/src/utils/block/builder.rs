use std::{borrow::Borrow, io, sync::Arc};

use crate::utils::block::Block;

trait PrimitiveBlockWriterImpl {
    fn write(&mut self, data: &[u8]) -> io::Result<()>;
    fn build_block(&mut self) -> io::Result<Block>;
}

impl<T> PrimitiveBlockWriterImpl for Box<T>
where
    T: PrimitiveBlockWriterImpl + ?Sized,
{
    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.as_mut().write(data)
    }

    fn build_block(&mut self) -> io::Result<Block> {
        self.as_mut().build_block()
    }
}

struct VecWriter(Vec<u8>);

impl PrimitiveBlockWriterImpl for VecWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.0.extend_from_slice(data);
        Ok(())
    }

    fn build_block(&mut self) -> io::Result<Block> {
        Ok(Block::from_vec(std::mem::take(&mut self.0)))
    }
}

pub struct BlockBuilder {
    writer_factory: Box<dyn FnMut() -> io::Result<Box<dyn PrimitiveBlockWriterImpl>>>,
    blocks: Vec<Block>,
    writer: Option<Box<dyn PrimitiveBlockWriterImpl>>,
}

macro_rules! impl_write_num {
    ($name:ident, $ty:ty) => {
        pub fn $name(&mut self, value: $ty) -> io::Result<&mut Self> {
            self.writer_inner()?.write(&value.to_le_bytes())?;
            Ok(self)
        }
    };
}

impl BlockBuilder {
    #[must_use]
    pub fn new_in_memory() -> Self {
        Self {
            writer_factory: Box::new(|| Ok(Box::new(VecWriter(Vec::new())))),
            blocks: Vec::new(),
            writer: None,
        }
    }

    fn writer_inner(&mut self) -> io::Result<&mut dyn PrimitiveBlockWriterImpl> {
        if self.writer.is_none() {
            self.writer = Some((self.writer_factory)()?);
        }
        Ok(self.writer.as_mut().unwrap())
    }

    fn flush_writer(&mut self) -> io::Result<()> {
        if let Some(mut writer) = self.writer.take() {
            self.blocks.push(writer.build_block()?);
        }
        Ok(())
    }

    impl_write_num!(write_u8, u8);
    impl_write_num!(write_u16_le, u16);
    impl_write_num!(write_u32_le, u32);
    impl_write_num!(write_u64_le, u64);
    impl_write_num!(write_i8, i8);
    impl_write_num!(write_i16_le, i16);
    impl_write_num!(write_i32_le, i32);
    impl_write_num!(write_i64_le, i64);
    impl_write_num!(write_f32_le, f32);
    impl_write_num!(write_f64_le, f64);

    pub fn write_block(&mut self, block: &Block) -> io::Result<&mut Self> {
        self.flush_writer()?;
        self.blocks.push(block.clone());
        Ok(self)
    }

    pub fn write_blocks(
        &mut self,
        blocks: impl IntoIterator<Item = impl Borrow<Block>>,
    ) -> io::Result<&mut Self> {
        for block in blocks {
            self.write_block(block.borrow())?;
        }
        Ok(self)
    }

    pub fn write_bytes(&mut self, data: &[u8]) -> io::Result<&mut Self> {
        self.writer_inner()?.write(data)?;
        Ok(self)
    }

    pub fn build(mut self) -> io::Result<Block> {
        self.flush_writer()?;
        Ok(Block::concat(self.blocks))
    }
}

#[derive(Clone)]
pub struct BlockBuilderFactory {
    factory: Arc<dyn Fn() -> BlockBuilder>,
}

impl BlockBuilderFactory {
    #[must_use]
    pub fn new_in_memory() -> Self {
        Self {
            factory: Arc::new(BlockBuilder::new_in_memory),
        }
    }

    #[must_use]
    pub fn create(&self) -> BlockBuilder {
        (self.factory)()
    }

    #[must_use]
    pub fn concat(
        &self,
        blocks: impl IntoIterator<Item = impl Borrow<Block>>,
    ) -> io::Result<Block> {
        let mut builder = self.create();
        builder.write_blocks(blocks)?;
        builder.build()
    }
}
