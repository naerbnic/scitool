use sci_utils::{
    block::{Block, BlockReader},
    buffer_ops::BufferOpsExt,
    data_reader::DataReader,
};

fn read_length_delimited_records(
    data: Block,
    record_size: u16,
) -> anyhow::Result<(Vec<Block>, Block)> {
    let num_records = data.read_u16_le_at(0);
    let (record_data, data) = data
        .subblock(2..)
        .split_at((num_records * record_size) as u64);
    let records = record_data.split_chunks(record_size as u64);
    Ok((records, data))
}

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
    relocations: Block,
    #[expect(dead_code)]
    exports: Vec<Block>,
    #[expect(dead_code)]
    thing2: Vec<Block>,
    #[expect(dead_code)]
    thing3: Vec<Block>,
    #[expect(dead_code)]
    rest: Block,
}

impl Script {
    pub fn from_block(data: Block) -> anyhow::Result<Self> {
        let relocation_offset = {
            let mut reader = BlockReader::new(data.clone());
            reader.read_u16_le()?
        };
        let (script_data, relocations) = data.split_at(relocation_offset as u64);
        let (exports, script_data) = read_length_delimited_records(script_data.subblock(2..), 2)?;
        let (thing2, script_data) = read_length_delimited_records(script_data, 2)?;
        let (thing3, rest) = read_length_delimited_records(script_data, 2)?;

        Ok(Self {
            data,
            relocations,
            exports,
            thing2,
            thing3,
            rest,
        })
    }
}
