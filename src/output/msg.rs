//! Data types used for output from scitool.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A message identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageId {
    pub room: u16,
    pub noun: u8,
    pub verb: u8,
    pub condition: u8,
    pub sequence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub talker: u8,
    pub text: String,
}

/// The top level structure for a message output file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFile {
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastEntry {
    pub name: String,
    pub short_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalkerEntry {
    pub id: u8,
    // A reference to a cast entry.
    pub cast: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbEntry {
    pub id: u8,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEntry {
    pub id: u16,
    pub name: String,
}

/// The top-level script config structure, and embedding in the messages file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptConfig {
    pub cast: BTreeMap<String, CastEntry>,
    pub talkers: Vec<TalkerEntry>,
    pub verbs: Vec<VerbEntry>,
    pub rooms: Vec<RoomEntry>,
}
