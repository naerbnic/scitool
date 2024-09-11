use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{RawConditionId, RawNounId, RawRoleId, RawRoomId, RawTalkerId, RawVerbId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RoleEntry {
    pub name: String,
    pub short_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct TalkerEntry {
    pub id: RawTalkerId,
    // A reference to a role entry.
    pub role: RawRoleId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct VerbEntry {
    pub id: RawVerbId,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ConditionEntry {
    pub id: RawConditionId,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct NounEntry {
    pub id: RawNounId,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RoomEntry {
    pub id: RawRoomId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<ConditionEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nouns: Vec<NounEntry>,
}

/// The top-level script config structure, and embedding in the messages file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookConfig {
    pub(super) roles: BTreeMap<RawRoleId, RoleEntry>,
    pub(super) talkers: Vec<TalkerEntry>,
    pub(super) verbs: Vec<VerbEntry>,
    pub(super) rooms: Vec<RoomEntry>,
}
