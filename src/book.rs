//! Code to manage the organization and generation of VO scripts,
//! (referred to as "books" to disambguate from script resources).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod builder;
pub mod config;

// Plain IDs.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RoomId(u16);

impl From<u16> for RoomId {
    fn from(value: u16) -> Self {
        RoomId(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NounId(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VerbId(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConditionId(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceId(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TalkerId(u8);

// Book Specific IDs.

/// An identifier for a cast member.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CastId(String);

pub struct Room<'a> {
    parent: &'a Book,
    id: RoomId,
    entry: &'a builder::RoomEntry,
}

pub struct CastMember<'a> {
    parent: &'a Book,
    id: CastId,
    entry: &'a builder::CastEntry,
}

pub struct Book {
    cast: BTreeMap<CastId, builder::CastEntry>,
    talkers: BTreeMap<TalkerId, builder::TalkerEntry>,
    verbs: BTreeMap<VerbId, builder::VerbEntry>,
    rooms: BTreeMap<RoomId, builder::RoomEntry>,
}

impl Book {
    pub fn rooms(&self) -> impl Iterator<Item = Room> {
        self.rooms.iter().map(|(k, v)| Room {
            parent: self,
            id: *k,
            entry: v,
        })
    }

    pub fn get_room(&self, id: RoomId) -> Option<Room> {
        self.rooms.get(&id).map(|entry| Room {
            parent: self,
            id,
            entry,
        })
    }

    pub fn cast_members(&self) -> &BTreeMap<CastId, builder::CastEntry> {
        &self.cast
    }
}
