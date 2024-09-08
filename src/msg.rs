use crate::util::{
    block::{Block, BlockReader},
    data_reader::DataReader,
};

pub fn parse_message_resource(msg_res: Block) -> anyhow::Result<()> {
    let mut reader = BlockReader::new(msg_res);
    let version_num = reader.read_u32_le()? / 1000;
    match version_num {
        _ => anyhow::bail!("Unsupported message resource version: {}", version_num),
    }
}
