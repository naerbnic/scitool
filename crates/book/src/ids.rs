use serde::{Deserialize, Serialize};

use scidev_common::RawVerbId;

// Raw IDs.
//
// There are the internal IDs used to reference different entities in the book.
// They are copyable, but only reference a single literal value from the SCI message
// file. They are used to construct the public IDs that are used to navigate the book.
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
pub struct VerbId(RawVerbId);

impl VerbId {
    #[must_use]
    pub fn from_raw(verb: RawVerbId) -> Self {
        VerbId(verb)
    }

    #[must_use]
    pub fn verb_num(&self) -> u8 {
        self.0.as_u8()
    }

    #[must_use]
    pub fn raw_id(&self) -> RawVerbId {
        self.0
    }
}

impl std::fmt::Debug for VerbId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("VerbId").field(&self.0.as_u8()).finish()
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoleId(RawRoleId);

impl RoleId {
    #[must_use]
    pub fn from_raw(raw_id: RawRoleId) -> RoleId {
        RoleId(raw_id)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0.0
    }

    #[must_use]
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
pub struct TalkerId(RawTalkerId);

impl TalkerId {
    pub fn from_raw(talker: RawTalkerId) -> Self {
        TalkerId(talker)
    }

    pub fn talker_num(self) -> u8 {
        self.0.0
    }

    pub fn raw_id(self) -> RawTalkerId {
        self.0
    }
}

impl std::fmt::Debug for TalkerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RoleId").field(&self.0.0).finish()
    }
}
