use sci_utils::{
    block::{Block, BlockReader},
    data_reader::DataReader,
};

struct Relocations {
    num_relocations: usize,
    reloc_block: Block,
}

impl Relocations {
    pub fn num_relocations(&self) -> usize {
        self.num_relocations
    }
}

pub struct Script {
    data: Block,
    relocations: Relocations,
    externs: Block,
    thing2: Block,
    thing3: Block,
}

impl Script {
    pub fn from_block(data: Block) -> anyhow::Result<Self> {
        let relocation_offset = {
            let mut reader = BlockReader::new(data.clone());
            reader.read_u16_le()?
        };
        let (script_data, relocation) = data.split_at(relocation_offset as u64);

        todo!()
    }
}
