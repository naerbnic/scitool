use std::collections::BTreeMap;

use sci_utils::{
    block::{BlockReader, MemBlock},
    buffer::BufferExt,
    data_reader::DataReader,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageId {
    noun: u8,
    verb: u8,
    condition: u8,
    sequence: u8,
}

impl MessageId {
    pub fn new(noun: u8, verb: u8, condition: u8, sequence: u8) -> Self {
        MessageId {
            noun,
            verb,
            condition,
            sequence,
        }
    }

    pub fn noun(&self) -> u8 {
        self.noun
    }

    pub fn verb(&self) -> u8 {
        self.verb
    }

    pub fn condition(&self) -> u8 {
        self.condition
    }

    pub fn sequence(&self) -> u8 {
        self.sequence
    }
}

#[derive(Debug, Clone, Copy)]
struct RawMessageRecord {
    id: MessageId,
    ref_id: MessageId,
    text_offset: u16,
    talker: u8,
}

#[derive(Debug)]
pub struct MessageRecord {
    _ref_id: MessageId,
    text: String,
    talker: u8,
}

impl MessageRecord {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn talker(&self) -> u8 {
        self.talker
    }
}

fn parse_message_resource_v4(msg_res: MemBlock) -> anyhow::Result<Vec<RawMessageRecord>> {
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

fn read_string_at_offset(msg_res: &MemBlock, offset: u16) -> anyhow::Result<String> {
    let mut reader = BlockReader::new(msg_res.clone().sub_buffer(offset as usize..));
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
    msg_res: &MemBlock,
    raw_record: RawMessageRecord,
) -> anyhow::Result<MessageRecord> {
    let text = read_string_at_offset(msg_res, raw_record.text_offset)?;
    Ok(MessageRecord {
        _ref_id: raw_record.ref_id,
        text,
        talker: raw_record.talker,
    })
}

pub struct RoomMessageSet {
    messages: BTreeMap<MessageId, MessageRecord>,
}

impl RoomMessageSet {
    pub fn messages(&self) -> impl Iterator<Item = (&MessageId, &MessageRecord)> {
        self.messages.iter()
    }
}

pub fn parse_message_resource(msg_res: MemBlock) -> anyhow::Result<RoomMessageSet> {
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
            Ok((raw_record.id, record))
        })
        .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
    Ok(RoomMessageSet { messages })
}
