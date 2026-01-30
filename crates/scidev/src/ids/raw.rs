use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawNounId(u8);

impl RawNounId {
    #[must_use]
    pub fn new(value: u8) -> Self {
        RawNounId(value)
    }

    pub fn number(self) -> u8 {
        self.0
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
    pub fn number(&self) -> u8 {
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

    pub fn number(self) -> u8 {
        self.0
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
    pub fn number(&self) -> u8 {
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

    pub fn number(self) -> u16 {
        self.0
    }
}

impl From<u16> for RawRoomId {
    fn from(value: u16) -> Self {
        RawRoomId(value)
    }
}
