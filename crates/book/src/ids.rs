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
// file. They are used to construct the public IDs that are used to navigate the book.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawRoomId(u16);

impl RawRoomId {
    pub fn new(value: u16) -> Self {
        RawRoomId(value)
    }
}

impl From<u16> for RawRoomId {
    fn from(value: u16) -> Self {
        RawRoomId(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawNounId(u8);

impl RawNounId {
    pub fn new(value: u8) -> Self {
        RawNounId(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawVerbId(u8);

impl RawVerbId {
    pub fn new(value: u8) -> Self {
        RawVerbId(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawConditionId(u8);

impl RawConditionId {
    pub fn new(value: u8) -> Self {
        RawConditionId(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawSequenceId(u8);

impl RawSequenceId {
    pub fn new(value: u8) -> Self {
        RawSequenceId(value)
    }

    pub fn as_u8(&self) -> u8 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawTalkerId(u8);

impl RawTalkerId {
    pub fn new(value: u8) -> Self {
        RawTalkerId(value)
    }
}

// Book Specific IDs.

/// An identifier for a role.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawRoleId(String);

// Public IDs.
//
// These uniquely identify different entities in the book. They are frequently
// composite ids, in order to navigate to the correct entity in the book.

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoomId(RawRoomId);

impl RoomId {
    pub fn from_raw(raw_id: RawRoomId) -> RoomId {
        RoomId(raw_id)
    }

    pub fn room_num(&self) -> u16 {
        self.0.0
    }

    pub fn raw_id(&self) -> RawRoomId {
        self.0
    }
}

impl Display for RoomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "room-{}", self.room_num())
    }
}

impl std::str::FromStr for RoomId {
    type Err = IdConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 || parts[0] != "room" {
            return Err(IdConversionError {
                message: format!("Invalid room ID format: {}", s),
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
pub struct VerbId(RawVerbId);

impl VerbId {
    pub fn from_raw(verb: RawVerbId) -> Self {
        VerbId(verb)
    }

    pub fn verb_num(&self) -> u8 {
        self.0.0
    }

    pub fn raw_id(&self) -> RawVerbId {
        self.0
    }
}

impl std::fmt::Debug for VerbId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("VerbId").field(&self.0.0).finish()
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoleId(RawRoleId);

impl RoleId {
    pub fn from_raw(raw_id: RawRoleId) -> RoleId {
        RoleId(raw_id)
    }
    pub fn as_str(&self) -> &str {
        &self.0.0
    }

    pub fn raw_id(&self) -> &RawRoleId {
        &self.0
    }
}

impl std::fmt::Debug for RoleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RoleId").field(&self.0.0).finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NounId(RoomId, RawNounId);

impl NounId {
    pub fn from_room_raw(id: RoomId, raw_id: RawNounId) -> NounId {
        NounId(id, raw_id)
    }

    pub fn room_id(&self) -> RoomId {
        self.0
    }

    pub fn raw_id(&self) -> RawNounId {
        self.1
    }

    pub fn room_num(&self) -> u16 {
        self.0.0.0
    }

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
                message: format!("Invalid noun ID format: {}", s),
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
pub struct TalkerId(RawTalkerId);

impl TalkerId {
    pub fn from_raw(talker: RawTalkerId) -> Self {
        TalkerId(talker)
    }

    pub fn talker_num(&self) -> u8 {
        self.0.0
    }

    pub fn raw_id(&self) -> RawTalkerId {
        self.0
    }
}

impl std::fmt::Debug for TalkerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RoleId").field(&self.0.0).finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConditionId(RoomId, RawConditionId);

impl ConditionId {
    pub fn from_room_raw(id: RoomId, raw_id: RawConditionId) -> ConditionId {
        ConditionId(id, raw_id)
    }

    pub fn room_id(&self) -> RoomId {
        self.0
    }

    pub fn raw_id(&self) -> RawConditionId {
        self.1
    }

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
    pub(super) fn new(verb: RawVerbId, condition: RawConditionId) -> Self {
        Self { verb, condition }
    }

    pub(super) fn verb(&self) -> RawVerbId {
        self.verb
    }

    pub(super) fn condition(&self) -> RawConditionId {
        self.condition
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConversationId(NounId, ConversationKey);

impl ConversationId {
    pub fn from_noun_key(noun_id: NounId, key: ConversationKey) -> Self {
        Self(noun_id, key)
    }

    pub fn noun_id(&self) -> NounId {
        self.0
    }

    pub fn conversation_key(&self) -> ConversationKey {
        self.1
    }

    pub fn room_num(&self) -> u16 {
        self.0.0.0.0
    }

    pub fn noun_num(&self) -> u8 {
        self.0.1.0
    }

    pub fn verb_num(&self) -> u8 {
        self.1.verb().0
    }

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
        if parts.len() != 4 || parts[0] != "conv" {
            return Err(IdConversionError {
                message: format!("Invalid conversation ID format: {}", s),
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
        Ok(ConversationId(
            NounId(RoomId(RawRoomId(room_num)), RawNounId(noun_num)),
            ConversationKey::new(RawVerbId(verb_num), RawConditionId(0)),
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
    pub fn from_conv_seq(conv_id: ConversationId, seq_id: RawSequenceId) -> Self {
        Self(conv_id, seq_id)
    }

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

    pub fn conv_id(&self) -> ConversationId {
        self.0
    }

    pub fn raw_id(&self) -> RawSequenceId {
        self.1
    }

    pub fn room_num(&self) -> u16 {
        self.0.room_num()
    }

    pub fn noun_num(&self) -> u8 {
        self.0.noun_num()
    }

    pub fn verb_num(&self) -> u8 {
        self.0.verb_num()
    }

    pub fn condition_num(&self) -> u8 {
        self.0.condition_num()
    }

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
        if parts.len() != 5 || parts[0] != "line" {
            return Err(IdConversionError {
                message: format!("Invalid line ID format: {}", s),
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
        Ok(LineId(
            ConversationId(
                NounId(RoomId(RawRoomId(room_num)), RawNounId(noun_num)),
                ConversationKey::new(RawVerbId(verb_num), RawConditionId(condition_num)),
            ),
            RawSequenceId(0),
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
