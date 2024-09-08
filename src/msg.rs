use std::collections::BTreeMap;

use crate::util::{
    block::{Block, BlockReader},
    data_reader::DataReader,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageId {
    noun: u8,
    verb: u8,
    condition: u8,
    sequence: u8,
}

#[derive(Debug, Clone, Copy)]
struct RawMessageRecord {
    id: MessageId,
    ref_id: MessageId,
    text_offset: u16,
    talker: u8,
}

#[expect(dead_code)]
#[derive(Debug)]
pub struct MessageRecord {
    ref_id: MessageId,
    text: String,
    talker: u8,
}

fn parse_message_resource_v4(msg_res: Block) -> anyhow::Result<Vec<RawMessageRecord>> {
    let mut reader = BlockReader::new(msg_res);
    let _header_data = reader.read_u32_le()?;
    let message_count = reader.read_u16_le()?;

    let mut raw_msg_records = Vec::new();
    for _ in 0..message_count {
        let id = {
            let noun = reader.read_u8()?;
            let verb = reader.read_u8()?;
            let condition = reader.read_u8()?;
            let sequence = reader.read_u8()?;
            MessageId {
                noun,
                verb,
                condition,
                sequence,
            }
        };

        let talker = reader.read_u8()?;
        let text_offset = reader.read_u16_le()?;

        let ref_id = {
            let noun = reader.read_u8()?;
            let verb = reader.read_u8()?;
            let condition = reader.read_u8()?;
            MessageId {
                noun,
                verb,
                condition,
                sequence: 1,
            }
        };

        // According to ScummVM, the record size is 11, but I don't know the purpose of
        // the last byte.
        let _unknown = reader.read_u8()?;

        let raw_record = RawMessageRecord {
            id,
            ref_id,
            text_offset,
            talker,
        };

        raw_msg_records.push(raw_record);
    }

    Ok(raw_msg_records)
}

fn read_string_at_offset(msg_res: &Block, offset: u16) -> anyhow::Result<String> {
    let mut reader = BlockReader::new(msg_res.subblock(offset as u64..));
    let mut text = Vec::new();
    loop {
        let ch = reader.read_u8()?;
        if ch == 0 {
            break;
        }
        text.push(ch);
    }
    Ok(String::from_utf8(text)?)
}

fn resolve_raw_record(
    msg_res: &Block,
    raw_record: RawMessageRecord,
) -> anyhow::Result<MessageRecord> {
    let text = read_string_at_offset(msg_res, raw_record.text_offset)?;
    Ok(MessageRecord {
        ref_id: raw_record.ref_id,
        text,
        talker: raw_record.talker,
    })
}

#[expect(dead_code)]
pub struct RoomMessageSet {
    messages: BTreeMap<MessageId, MessageRecord>,
}

pub fn parse_message_resource(msg_res: Block) -> anyhow::Result<RoomMessageSet> {
    let mut reader = BlockReader::new(msg_res.clone());
    let version_num = reader.read_u32_le()? / 1000;
    let raw_records = match version_num {
        4 => parse_message_resource_v4(reader.into_rest())?,
        _ => anyhow::bail!("Unsupported message resource version: {}", version_num),
    };

    let messages = raw_records
        .into_iter()
        .map(|raw_record| {
            let record = resolve_raw_record(&msg_res, raw_record)?;
            println!("{:?}: {:?}", raw_record.id, record);
            Ok((raw_record.id, record))
        })
        .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
    Ok(RoomMessageSet { messages })
}
