use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{RoleId, ConditionId, NounId, RoomId, TalkerId, VerbId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleEntry {
    pub name: String,
    pub short_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalkerEntry {
    pub id: TalkerId,
    // A reference to a role entry.
    pub role: RoleId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbEntry {
    pub id: VerbId,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionEntry {
    pub id: ConditionId,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NounEntry {
    pub id: NounId,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEntry {
    pub id: RoomId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<ConditionEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nouns: Vec<NounEntry>,
}

/// The top-level script config structure, and embedding in the messages file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookConfig {
    pub roles: BTreeMap<RoleId, RoleEntry>,
    pub talkers: Vec<TalkerEntry>,
    pub verbs: Vec<VerbEntry>,
    pub rooms: Vec<RoomEntry>,
}
