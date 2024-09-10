use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
pub struct BookConfig {
    pub cast: BTreeMap<String, CastEntry>,
    pub talkers: Vec<TalkerEntry>,
    pub verbs: Vec<VerbEntry>,
    pub rooms: Vec<RoomEntry>,
}
