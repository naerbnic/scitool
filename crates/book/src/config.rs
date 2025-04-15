use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::ids::{RawConditionId, RawNounId, RawRoleId, RawRoomId, RawTalkerId, RawVerbId};

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
    /// If true, this noun refers to a cutscene. It should have
    /// only one conversation with verb 0, and condition 0.
    #[serde(default)]
    pub is_cutscene: bool,
    /// If true, this room is not included in the final script.
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RoomEntry {
    /// The numeric room ID.
    pub id: RawRoomId,
    /// The human readable name of the room.
    pub name: String,
    /// The list of conditions used in the room.
    ///
    /// This should generally be a map from condition IDs to data, but
    /// there are several formats where numeric keys are not natively supported
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<ConditionEntry>,
    /// The list of nouns used in the room.
    ///
    /// This should generally be a map from noun IDs to data, but
    /// there are several formats where numeric keys are not natively supported
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nouns: Vec<NounEntry>,
    /// If true, this room is not included in the final script.
    #[serde(default)]
    pub hidden: bool,
}

/// The top-level script config structure, and embedding in the messages file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookConfig {
    pub(super) project_name: String,
    pub(super) roles: BTreeMap<RawRoleId, RoleEntry>,
    pub(super) talkers: Vec<TalkerEntry>,
    pub(super) verbs: Vec<VerbEntry>,
    pub(super) rooms: Vec<RoomEntry>,
}
