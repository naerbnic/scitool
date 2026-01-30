//! Common types that are used by all libraries. This should do no IO, or
//! interpretetation of existing data.

pub mod raw;

use std::{fmt::Display, str::FromStr};

use crate::ids::raw::{RawConditionId, RawNounId, RawRoomId, RawSequenceId, RawVerbId};

#[derive(thiserror::Error, Debug)]
#[error("Error converting to ID: {message}")]
pub struct IdConversionError {
    message: String,
}

// Raw IDs.
//
// There are the internal IDs used to reference different entities in the book.
// They are copyable, but only reference a single literal value from the SCI message
// file.

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoomId(raw::RawRoomId);

impl RoomId {
    #[must_use]
    pub fn from_raw(raw_id: raw::RawRoomId) -> RoomId {
        RoomId(raw_id)
    }

    #[must_use]
    pub fn room_num(&self) -> u16 {
        self.0.number()
    }

    #[must_use]
    pub fn raw_id(&self) -> raw::RawRoomId {
        self.0
    }
}

impl Display for RoomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "room-{}", self.room_num())
    }
}

impl FromStr for RoomId {
    type Err = IdConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 || parts[0] != "room" {
            return Err(IdConversionError {
                message: format!("Invalid room ID format: {s}"),
            });
        }
        let room_num = parts[1].parse::<u16>().map_err(|_| IdConversionError {
            message: format!("Invalid room number: {}", parts[1]),
        })?;
        Ok(RoomId(raw::RawRoomId::new(room_num)))
    }
}

impl std::fmt::Debug for RoomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RoomId").field(&self.0.number()).finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NounId(RoomId, raw::RawNounId);

impl NounId {
    #[must_use]
    pub fn from_room_raw(id: RoomId, raw_id: RawNounId) -> NounId {
        NounId(id, raw_id)
    }

    #[must_use]
    pub fn room_id(&self) -> RoomId {
        self.0
    }

    #[must_use]
    pub fn raw_id(&self) -> RawNounId {
        self.1
    }

    #[must_use]
    pub fn room_num(&self) -> u16 {
        self.0.0.number()
    }

    #[must_use]
    pub fn noun_num(&self) -> u8 {
        self.1.number()
    }
}

impl Display for NounId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "noun-{}-{}", self.room_num(), self.noun_num())
    }
}

impl FromStr for NounId {
    type Err = IdConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 || parts[0] != "noun" {
            return Err(IdConversionError {
                message: format!("Invalid noun ID format: {s}"),
            });
        }
        let room_num = parts[1].parse::<u16>().map_err(|_| IdConversionError {
            message: format!("Invalid room number: {}", parts[1]),
        })?;
        let noun_num = parts[2].parse::<u8>().map_err(|_| IdConversionError {
            message: format!("Invalid noun number: {}", parts[2]),
        })?;
        Ok(NounId(
            RoomId(RawRoomId::new(room_num)),
            RawNounId::new(noun_num),
        ))
    }
}

impl std::fmt::Debug for NounId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NounId")
            .field("room", &self.room_num())
            .field("noun", &self.noun_num())
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConditionId(RoomId, RawConditionId);

impl ConditionId {
    #[must_use]
    pub fn from_room_raw(id: RoomId, raw_id: RawConditionId) -> ConditionId {
        ConditionId(id, raw_id)
    }

    #[must_use]
    pub fn room_id(&self) -> RoomId {
        self.0
    }

    #[must_use]
    pub fn raw_id(&self) -> RawConditionId {
        self.1
    }

    #[must_use]
    pub fn condition_num(&self) -> u8 {
        self.1.number()
    }
}

impl std::fmt::Debug for ConditionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConditionId")
            .field("room", &self.0.0.number())
            .field("condition", &self.1.number())
            .finish()
    }
}

/// A key for a conversation in a noun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConversationKey {
    verb: RawVerbId,
    condition: RawConditionId,
}

impl ConversationKey {
    #[must_use]
    pub fn new(verb: RawVerbId, condition: RawConditionId) -> Self {
        Self { verb, condition }
    }

    #[must_use]
    pub fn verb(&self) -> RawVerbId {
        self.verb
    }

    #[must_use]
    pub fn condition(&self) -> RawConditionId {
        self.condition
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConversationId(NounId, ConversationKey);

impl ConversationId {
    #[must_use]
    pub fn from_noun_key(noun_id: NounId, key: ConversationKey) -> Self {
        Self(noun_id, key)
    }

    #[must_use]
    pub fn noun_id(&self) -> NounId {
        self.0
    }

    #[must_use]
    pub fn conversation_key(&self) -> ConversationKey {
        self.1
    }

    #[must_use]
    pub fn room_num(&self) -> u16 {
        self.0.0.0.number()
    }

    #[must_use]
    pub fn noun_num(&self) -> u8 {
        self.0.1.number()
    }

    #[must_use]
    pub fn verb_num(&self) -> u8 {
        self.1.verb().number()
    }

    #[must_use]
    pub fn condition_num(&self) -> u8 {
        self.1.condition().number()
    }
}

impl Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "conv-{}-{}-{}-{}",
            self.room_num(),
            self.noun_num(),
            self.verb_num(),
            self.condition_num()
        )
    }
}
impl FromStr for ConversationId {
    type Err = IdConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 5 || parts[0] != "conv" {
            return Err(IdConversionError {
                message: format!("Invalid conversation ID format: {s}"),
            });
        }
        let room_num = parts[1].parse::<u16>().map_err(|_| IdConversionError {
            message: format!("Invalid room number: {}", parts[1]),
        })?;
        let noun_num = parts[2].parse::<u8>().map_err(|_| IdConversionError {
            message: format!("Invalid noun number: {}", parts[2]),
        })?;
        let verb_num = parts[3].parse::<u8>().map_err(|_| IdConversionError {
            message: format!("Invalid verb number: {}", parts[3]),
        })?;
        let condition_num = parts[4].parse::<u8>().map_err(|_| IdConversionError {
            message: format!("Invalid condition number: {}", parts[4]),
        })?;
        Ok(ConversationId(
            NounId(RoomId(RawRoomId::new(room_num)), RawNounId::new(noun_num)),
            ConversationKey::new(RawVerbId::new(verb_num), RawConditionId::new(condition_num)),
        ))
    }
}

impl std::fmt::Debug for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversationId")
            .field("room", &self.room_num())
            .field("noun", &self.noun_num())
            .field("verb", &self.verb_num())
            .field("condition", &self.condition_num())
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineId(ConversationId, RawSequenceId);

impl LineId {
    #[must_use]
    pub fn from_conv_seq(conv_id: ConversationId, seq_id: RawSequenceId) -> Self {
        Self(conv_id, seq_id)
    }

    #[must_use]
    pub fn from_parts(
        room: raw::RawRoomId,
        noun: RawNounId,
        verb: RawVerbId,
        condition: RawConditionId,
        sequence: RawSequenceId,
    ) -> Self {
        LineId(
            ConversationId(
                NounId(RoomId(room), noun),
                ConversationKey::new(verb, condition),
            ),
            sequence,
        )
    }

    #[must_use]
    pub fn conv_id(&self) -> ConversationId {
        self.0
    }

    #[must_use]
    pub fn raw_id(&self) -> RawSequenceId {
        self.1
    }

    #[must_use]
    pub fn room_num(&self) -> u16 {
        self.0.room_num()
    }

    #[must_use]
    pub fn noun_num(&self) -> u8 {
        self.0.noun_num()
    }

    #[must_use]
    pub fn verb_num(&self) -> u8 {
        self.0.verb_num()
    }

    #[must_use]
    pub fn condition_num(&self) -> u8 {
        self.0.condition_num()
    }

    #[must_use]
    pub fn sequence_num(&self) -> u8 {
        self.1.number()
    }
}

impl Display for LineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line-{}-{}-{}-{}-{}",
            self.room_num(),
            self.noun_num(),
            self.verb_num(),
            self.condition_num(),
            self.sequence_num()
        )
    }
}

impl FromStr for LineId {
    type Err = IdConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 6 || parts[0] != "line" {
            return Err(IdConversionError {
                message: format!("Invalid line ID format: {s}"),
            });
        }
        let room_num = parts[1].parse::<u16>().map_err(|_| IdConversionError {
            message: format!("Invalid room number: {}", parts[1]),
        })?;
        let noun_num = parts[2].parse::<u8>().map_err(|_| IdConversionError {
            message: format!("Invalid noun number: {}", parts[2]),
        })?;
        let verb_num = parts[3].parse::<u8>().map_err(|_| IdConversionError {
            message: format!("Invalid verb number: {}", parts[3]),
        })?;
        let condition_num = parts[4].parse::<u8>().map_err(|_| IdConversionError {
            message: format!("Invalid condition number: {}", parts[4]),
        })?;
        let sequence_num = parts[5].parse::<u8>().map_err(|_| IdConversionError {
            message: format!("Invalid sequence number: {}", parts[5]),
        })?;
        Ok(LineId(
            ConversationId(
                NounId(RoomId(RawRoomId::new(room_num)), RawNounId::new(noun_num)),
                ConversationKey::new(RawVerbId::new(verb_num), RawConditionId::new(condition_num)),
            ),
            RawSequenceId::new(sequence_num),
        ))
    }
}

impl std::fmt::Debug for LineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineId")
            .field("room", &self.0.0.0.0.number())
            .field("noun", &self.0.0.1.number())
            .field("verb", &self.0.1.verb().number())
            .field("condition", &self.0.1.condition().number())
            .field("sequence", &self.1.number())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_id() {
        let line_id = LineId::from_parts(
            raw::RawRoomId::new(1),
            RawNounId::new(2),
            RawVerbId::new(3),
            RawConditionId::new(4),
            RawSequenceId::new(5),
        );
        assert_eq!(line_id.room_num(), 1);
        assert_eq!(line_id.noun_num(), 2);
        assert_eq!(line_id.verb_num(), 3);
        assert_eq!(line_id.condition_num(), 4);
        assert_eq!(line_id.sequence_num(), 5);
    }

    #[test]
    fn test_line_id_string_roundtrip() {
        let line_id = LineId::from_parts(
            raw::RawRoomId::new(1),
            RawNounId::new(2),
            RawVerbId::new(3),
            RawConditionId::new(4),
            RawSequenceId::new(5),
        );
        let line_id_str = line_id.to_string();
        let parsed_line_id = LineId::from_str(&line_id_str).unwrap();
        assert_eq!(line_id, parsed_line_id);
    }

    #[test]
    fn test_conv_id_string_roundtrip() {
        let conv_id = ConversationId::from_noun_key(
            NounId(RoomId(raw::RawRoomId::new(1)), RawNounId::new(2)),
            ConversationKey::new(RawVerbId::new(3), RawConditionId::new(4)),
        );
        let conv_id_str = conv_id.to_string();
        let parsed_conv_id = ConversationId::from_str(&conv_id_str).unwrap();
        assert_eq!(conv_id, parsed_conv_id);
    }
}
