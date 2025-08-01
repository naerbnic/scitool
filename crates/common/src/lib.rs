//! Common types that are used by all libraries. This should do no IO, or
//! interpretetation of existing data.

use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawNounId(u8);

impl RawNounId {
    #[must_use]
    pub fn new(value: u8) -> Self {
        RawNounId(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawVerbId(u8);

impl RawVerbId {
    #[must_use]
    pub fn new(value: u8) -> Self {
        RawVerbId(value)
    }

    #[must_use]
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawConditionId(u8);

impl RawConditionId {
    #[must_use]
    pub fn new(value: u8) -> Self {
        RawConditionId(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawSequenceId(u8);

impl RawSequenceId {
    #[must_use]
    pub fn new(value: u8) -> Self {
        RawSequenceId(value)
    }

    #[must_use]
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawRoomId(u16);

impl RawRoomId {
    #[must_use]
    pub fn new(value: u16) -> Self {
        RawRoomId(value)
    }
}

impl From<u16> for RawRoomId {
    fn from(value: u16) -> Self {
        RawRoomId(value)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoomId(RawRoomId);

impl RoomId {
    #[must_use]
    pub fn from_raw(raw_id: RawRoomId) -> RoomId {
        RoomId(raw_id)
    }

    #[must_use]
    pub fn room_num(&self) -> u16 {
        self.0.0
    }

    #[must_use]
    pub fn raw_id(&self) -> RawRoomId {
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
        Ok(RoomId(RawRoomId(room_num)))
    }
}

impl std::fmt::Debug for RoomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RoomId").field(&self.0.0).finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NounId(RoomId, RawNounId);

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
        self.0.0.0
    }

    #[must_use]
    pub fn noun_num(&self) -> u8 {
        self.1.0
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
        Ok(NounId(RoomId(RawRoomId(room_num)), RawNounId(noun_num)))
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
        self.1.0
    }
}

impl std::fmt::Debug for ConditionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConditionId")
            .field("room", &self.0.0.0)
            .field("condition", &self.1.0)
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
        self.0.0.0.0
    }

    #[must_use]
    pub fn noun_num(&self) -> u8 {
        self.0.1.0
    }

    #[must_use]
    pub fn verb_num(&self) -> u8 {
        self.1.verb().0
    }

    #[must_use]
    pub fn condition_num(&self) -> u8 {
        self.1.condition().0
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
            NounId(RoomId(RawRoomId(room_num)), RawNounId(noun_num)),
            ConversationKey::new(RawVerbId(verb_num), RawConditionId(condition_num)),
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
        room: RawRoomId,
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
        self.1.0
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
                NounId(RoomId(RawRoomId(room_num)), RawNounId(noun_num)),
                ConversationKey::new(RawVerbId(verb_num), RawConditionId(condition_num)),
            ),
            RawSequenceId(sequence_num),
        ))
    }
}

impl std::fmt::Debug for LineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineId")
            .field("room", &self.0.0.0.0.0)
            .field("noun", &self.0.0.1.0)
            .field("verb", &self.0.1.verb().0)
            .field("condition", &self.0.1.condition().0)
            .field("sequence", &self.1.0)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_id() {
        let line_id = LineId::from_parts(
            RawRoomId::new(1),
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
            RawRoomId::new(1),
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
            NounId(RoomId(RawRoomId::new(1)), RawNounId::new(2)),
            ConversationKey::new(RawVerbId::new(3), RawConditionId::new(4)),
        );
        let conv_id_str = conv_id.to_string();
        let parsed_conv_id = ConversationId::from_str(&conv_id_str).unwrap();
        assert_eq!(conv_id, parsed_conv_id);
    }
}
