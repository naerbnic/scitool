use sci_utils::{
    block::{Block, BlockReader},
    data_reader::DataReader,
};

struct Relocations {
    num_relocations: usize,
    #[expect(dead_code)]
    reloc_block: Block,
}

impl Relocations {
    #[expect(dead_code)]
    pub fn num_relocations(&self) -> usize {
        self.num_relocations
    }
}

pub struct Script {
    #[expect(dead_code)]
    data: Block,
    #[expect(dead_code)]
    relocations: Relocations,
    #[expect(dead_code)]
    externs: Block,
    #[expect(dead_code)]
    thing2: Block,
    #[expect(dead_code)]
    thing3: Block,
}

impl Script {
    pub fn from_block(data: Block) -> anyhow::Result<Self> {
        let relocation_offset = {
            let mut reader = BlockReader::new(data.clone());
            reader.read_u16_le()?
        };
        let (_, _) = data.split_at(relocation_offset as u64);

        todo!()
    }
}
