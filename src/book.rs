//! Code to manage the organization and generation of VO scripts,
//! (referred to as "books" to disambguate from script resources).

use std::collections::BTreeMap;

use builder::ConversationKey;
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConversationId(RoomId, NounId, VerbId, ConditionId);

pub struct ConditionInfo {
    /// If this was configured with a description in the input config file,
    /// this will be Some.
    #[expect(dead_code)]
    builder: Option<builder::ConditionEntry>,
}

pub struct LineEntry {
    #[expect(dead_code)]
    text: String,
    #[expect(dead_code)]
    talker: TalkerId,
}

pub struct ConversationEntry {
    lines: BTreeMap<SequenceId, LineEntry>,
}

pub struct NounEntry {
    #[expect(dead_code)]
    desc: Option<String>,
    conversations: BTreeMap<ConversationKey, ConversationEntry>,
}

pub struct RoomEntry {
    #[expect(dead_code)]
    name: String,
    #[expect(dead_code)]
    conditions: BTreeMap<ConditionId, ConditionInfo>,
    nouns: BTreeMap<NounId, NounEntry>,
}

pub struct CastMemberEntry {
    #[expect(dead_code)]
    name: String,
    #[expect(dead_code)]
    short_name: String,
}

pub struct TalkerEntry {
    #[expect(dead_code)]
    cast_id: CastId,
}

pub struct VerbEntry {
    #[expect(dead_code)]
    name: String,
}

pub struct Line<'a> {
    #[expect(dead_code)]
    book: &'a Book,
    #[expect(dead_code)]
    room: &'a RoomEntry,
    #[expect(dead_code)]
    noun: &'a NounEntry,
    #[expect(dead_code)]
    conversation: &'a ConversationEntry,
    #[expect(dead_code)]
    line: &'a LineEntry,
    #[expect(dead_code)]
    room_id: RoomId,
    #[expect(dead_code)]
    noun_id: NounId,
    #[expect(dead_code)]
    conversation_key: ConversationKey,
    #[expect(dead_code)]
    sequence_id: SequenceId,
}

pub struct Conversation<'a> {
    book: &'a Book,
    room: &'a RoomEntry,
    noun: &'a NounEntry,
    conversation: &'a ConversationEntry,
    room_id: RoomId,
    noun_id: NounId,
    conversation_key: ConversationKey,
}

impl<'a> Conversation<'a> {
    #[expect(dead_code)]
    pub fn lines(&self) -> impl Iterator<Item = Line> {
        self.conversation.lines.iter().map(|(k, v)| Line {
            book: self.book,
            room: self.room,
            noun: self.noun,
            conversation: self.conversation,
            line: v,
            room_id: self.room_id,
            noun_id: self.noun_id,
            conversation_key: self.conversation_key,
            sequence_id: *k,
        })
    }
}

pub struct Verb<'a> {
    #[expect(dead_code)]
    book: &'a Book,
    #[expect(dead_code)]
    verb: &'a VerbEntry,
    #[expect(dead_code)]
    id: VerbId,
}

pub struct Talker<'a> {
    #[expect(dead_code)]
    book: &'a Book,
    #[expect(dead_code)]
    talker: &'a TalkerEntry,
    #[expect(dead_code)]
    id: TalkerId,
}

pub struct Noun<'a> {
    book: &'a Book,
    room: &'a RoomEntry,
    #[expect(dead_code)]
    noun: &'a NounEntry,
    room_id: RoomId,
    noun_id: NounId,
    entry: &'a NounEntry,
}

impl<'a> Noun<'a> {
    #[expect(dead_code)]
    pub fn conversations(&self) -> impl Iterator<Item = Conversation> {
        self.entry.conversations.iter().map(|(k, v)| Conversation {
            book: self.book,
            room: self.room,
            noun: self.entry,
            conversation: v,
            room_id: self.room_id,
            noun_id: self.noun_id,
            conversation_key: *k,
        })
    }
}

pub struct Room<'a> {
    parent: &'a Book,
    id: RoomId,
    entry: &'a RoomEntry,
}

impl<'a> Room<'a> {
    #[expect(dead_code)]
    pub fn nouns(&self) -> impl Iterator<Item = Noun> {
        self.entry.nouns.iter().map(|(k, v)| Noun {
            book: self.parent,
            room: self.entry,
            room_id: self.id,
            noun_id: *k,
            noun: v,
            entry: v,
        })
    }
}

pub struct CastMember<'a> {
    #[expect(dead_code)]
    parent: &'a Book,
    #[expect(dead_code)]
    id: &'a CastId,
    #[expect(dead_code)]
    entry: &'a CastMemberEntry,
}

pub struct Book {
    cast: BTreeMap<CastId, CastMemberEntry>,
    talkers: BTreeMap<TalkerId, TalkerEntry>,
    verbs: BTreeMap<VerbId, VerbEntry>,
    rooms: BTreeMap<RoomId, RoomEntry>,
}

impl Book {
    #[expect(dead_code)]
    pub fn rooms(&self) -> impl Iterator<Item = Room> {
        self.rooms.iter().map(|(k, v)| Room {
            parent: self,
            id: *k,
            entry: v,
        })
    }

    #[expect(dead_code)]
    pub fn get_room(&self, id: RoomId) -> Option<Room> {
        self.rooms.get(&id).map(|entry| Room {
            parent: self,
            id,
            entry,
        })
    }

    #[expect(dead_code)]
    pub fn cast_members(&self) -> impl Iterator<Item = CastMember> {
        self.cast.iter().map(|(k, v)| CastMember {
            parent: self,
            id: k,
            entry: v,
        })
    }

    #[expect(dead_code)]
    pub fn get_cast_member(&self, id: &CastId) -> Option<CastMember> {
        self.cast.get_key_value(id).map(|(k, entry)| CastMember {
            parent: self,
            id: k,
            entry,
        })
    }

    #[expect(dead_code)]
    pub fn verbs(&self) -> impl Iterator<Item = Verb> {
        self.verbs.iter().map(|(k, v)| Verb {
            book: self,
            id: *k,
            verb: v,
        })
    }

    #[expect(dead_code)]
    pub fn get_verb(&self, id: VerbId) -> Option<Verb> {
        self.verbs.get(&id).map(|entry| Verb {
            book: self,
            id,
            verb: entry,
        })
    }

    #[expect(dead_code)]
    pub fn talkers(&self) -> impl Iterator<Item = Talker> {
        self.talkers.iter().map(|(k, v)| Talker {
            book: self,
            id: *k,
            talker: v,
        })
    }

    #[expect(dead_code)]
    pub fn get_talker(&self, id: TalkerId) -> Option<Talker> {
        self.talkers.get(&id).map(|entry| Talker {
            book: self,
            id,
            talker: entry,
        })
    }
}
