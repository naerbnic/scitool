use std::collections::BTreeMap;

use crate::utils::{
    block::MemBlock,
    buffer::BufferExt,
    errors::{OtherError, bail_other, ensure_other, prelude::*},
    mem_reader::{BufferMemReader, MemReader},
};

use serde::{Deserialize, Serialize};

fn zero_u8() -> u8 {
    0
}

fn one_u8() -> u8 {
    1
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Required by serde skip_serializing_if attribute"
)]
fn is_zero_u8(x: &u8) -> bool {
    *x == 0
}
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Required by serde skip_serializing_if attribute"
)]
fn is_one_u8(x: &u8) -> bool {
    *x == 1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MessageId {
    noun: u8,
    #[serde(default = "zero_u8", skip_serializing_if = "is_zero_u8")]
    verb: u8,
    #[serde(default = "zero_u8", skip_serializing_if = "is_zero_u8")]
    condition: u8,
    #[serde(default = "one_u8", skip_serializing_if = "is_one_u8")]
    sequence: u8,
}

impl MessageId {
    pub fn new(
        noun: u8,
        verb: impl Into<Option<u8>>,
        condition: impl Into<Option<u8>>,
        sequence: impl Into<Option<u8>>,
    ) -> Self {
        MessageId {
            noun,
            verb: verb.into().unwrap_or(0),
            condition: condition.into().unwrap_or(0),
            sequence: sequence.into().unwrap_or(1),
        }
    }

    #[must_use]
    pub fn noun(&self) -> u8 {
        self.noun
    }

    #[must_use]
    pub fn verb(&self) -> u8 {
        self.verb
    }

    #[must_use]
    pub fn condition(&self) -> u8 {
        self.condition
    }

    #[must_use]
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
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn talker(&self) -> u8 {
        self.talker
    }
}

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct ParseError(#[from] OtherError);

fn parse_message_resource_v4<'a, M: MemReader<'a>>(
    mut reader: M,
) -> Result<Vec<RawMessageRecord>, ParseError> {
    let _header_data = reader.read_u32_le().with_other_err()?;
    let message_count = reader.read_u16_le().with_other_err()?;

    let mut raw_msg_records = Vec::new();
    for _ in 0..message_count {
        let id = {
            let noun = reader.read_u8().with_other_err()?;
            let verb = reader.read_u8().with_other_err()?;
            let condition = reader.read_u8().with_other_err()?;
            let sequence = reader.read_u8().with_other_err()?;
            MessageId::new(noun, verb, condition, sequence)
        };

        let talker = reader.read_u8().with_other_err()?;
        let text_offset = reader.read_u16_le().with_other_err()?;

        let ref_id = {
            let noun = reader.read_u8().with_other_err()?;
            let verb = reader.read_u8().with_other_err()?;
            let condition = reader.read_u8().with_other_err()?;
            MessageId::new(noun, verb, condition, None)
        };

        // According to ScummVM, the record size is 11, but I don't know the purpose of
        // the last byte.
        let _unknown = reader.read_u8().with_other_err()?;

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

fn read_string_at_offset(msg_res: &MemBlock, offset: u16) -> Result<String, ParseError> {
    ensure_other!(
        offset as usize <= msg_res.size(),
        "String offset out of bounds"
    );
    let base_buffer = msg_res.clone().sub_buffer(offset as usize..);
    let mut reader = BufferMemReader::new(&base_buffer);
    let mut text = Vec::new();
    loop {
        let ch = reader.read_u8().with_other_err()?;
        if ch == 0 {
            break;
        }
        text.push(ch);
    }
    Ok(String::from_utf8(text).with_other_err()?)
}

fn resolve_raw_record(
    msg_res: &MemBlock,
    raw_record: RawMessageRecord,
) -> Result<MessageRecord, ParseError> {
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
    pub fn messages(&self) -> impl Iterator<Item = (MessageId, &MessageRecord)> {
        self.messages.iter().map(|(&id, record)| (id, record))
    }
}

pub fn parse_message_resource(msg_res: &MemBlock) -> Result<RoomMessageSet, ParseError> {
    let mut reader = BufferMemReader::new(msg_res);
    let version_num = reader.read_u32_le().with_other_err()? / 1000;
    let raw_records = match version_num {
        4 => parse_message_resource_v4(reader)?,
        _ => bail_other!("Unsupported message resource version: {}", version_num),
    };

    let messages = raw_records
        .into_iter()
        .map(|raw_record| {
            let record = resolve_raw_record(msg_res, raw_record)?;
            Ok((raw_record.id, record))
        })
        .collect::<Result<BTreeMap<_, _>, ParseError>>()?;
    Ok(RoomMessageSet { messages })
}
