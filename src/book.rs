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

/// An identifier for a role.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RoleId(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConversationId(RoomId, NounId, VerbId, ConditionId);

// Entries
//
// These are the actual data structures that are stored in the book.
// They form a tree of data that can be navigated to find the specific
// information needed from the book.
//
// Public access is provided by the handle types below.

struct ConditionEntry {
    /// If this was configured with a description in the input config file,
    /// this will be Some.
    builder: Option<builder::ConditionEntry>,
}

struct LineEntry {
    text: String,
    talker: TalkerId,
}

struct ConversationEntry {
    lines: BTreeMap<SequenceId, LineEntry>,
}

struct NounEntry {
    desc: Option<String>,
    conversations: BTreeMap<ConversationKey, ConversationEntry>,
}

struct RoomEntry {
    name: String,
    conditions: BTreeMap<ConditionId, ConditionEntry>,
    nouns: BTreeMap<NounId, NounEntry>,
}

struct RoleEntry {
    name: String,
    short_name: String,
}

struct TalkerEntry {
    role_id: RoleId,
}

struct VerbEntry {
    name: String,
}

// Handles
//
// These are the public types that are used to navigate the book.
// They provide methods that let you access different related
// entities in the book, for instance, which conversations have
// which roles in them.
//
// They all borrow from the book instance itself.

pub struct Line<'a> {
    book: &'a Book,
    room: &'a RoomEntry,
    noun: &'a NounEntry,
    conversation: &'a ConversationEntry,
    line: &'a LineEntry,
    room_id: RoomId,
    noun_id: NounId,
    conversation_key: ConversationKey,
    #[expect(dead_code)]
    sequence_id: SequenceId,
}

impl<'a> Line<'a> {
    #[expect(dead_code)]
    pub fn text(&self) -> &str {
        &self.line.text
    }

    pub fn talker(&self) -> Talker<'a> {
        self.book.get_talker(self.line.talker).unwrap()
    }

    #[expect(dead_code)]
    pub fn role(&self) -> Role<'a> {
        self.talker().role()
    }

    #[expect(dead_code)]
    pub fn conversation(&self) -> Conversation<'a> {
        Conversation {
            book: self.book,
            room: self.room,
            noun: self.noun,
            conversation: self.conversation,
            room_id: self.room_id,
            noun_id: self.noun_id,
            conversation_key: self.conversation_key,
        }
    }
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

    /// Get the noun this conversation is part of.
    #[expect(dead_code)]
    pub fn noun(&self) -> Noun<'a> {
        Noun {
            book: self.book,
            room: self.room,
            noun: self.noun,
            room_id: self.room_id,
            noun_id: self.noun_id,
        }
    }

    /// Get the verb used for this conversation (if it exists).
    #[expect(dead_code)]
    pub fn verb(&self) -> Option<Verb<'a>> {
        if self.conversation_key.verb() == VerbId(0) {
            return None;
        }
        Some(self.book.get_verb(self.conversation_key.verb()).unwrap())
    }

    /// Get the condition needed for this conversation (if it exists).
    #[expect(dead_code)]
    pub fn condition(&self) -> Option<Condition<'a>> {
        if self.conversation_key.condition() == ConditionId(0) {
            return None;
        }
        Some(Condition {
            book: self.book,
            room: self.room,
            room_id: self.room_id,
            id: self.conversation_key.condition(),
            condition: self
                .room
                .conditions
                .get(&self.conversation_key.condition())
                .unwrap(),
        })
    }
}

pub struct Condition<'a> {
    book: &'a Book,
    room: &'a RoomEntry,
    condition: &'a ConditionEntry,
    room_id: RoomId,
    #[expect(dead_code)]
    id: ConditionId,
}

impl<'a> Condition<'a> {
    /// Get the description of this condition (if specified).
    #[expect(dead_code)]
    pub fn desc(&self) -> Option<&str> {
        self.condition.builder.as_ref().map(|b| b.desc())
    }

    /// Get the room this condition is part of.
    #[expect(dead_code)]
    pub fn room(&self) -> Room<'a> {
        Room {
            parent: self.book,
            id: self.room_id,
            entry: self.room,
        }
    }
}

pub struct Verb<'a> {
    #[expect(dead_code)]
    book: &'a Book,
    verb: &'a VerbEntry,
    #[expect(dead_code)]
    id: VerbId,
}

impl<'a> Verb<'a> {
    #[expect(dead_code)]
    pub fn name(&self) -> &str {
        &self.verb.name
    }
}

pub struct Talker<'a> {
    book: &'a Book,
    talker: &'a TalkerEntry,
    #[expect(dead_code)]
    id: TalkerId,
}

impl<'a> Talker<'a> {
    pub fn role(&self) -> Role<'a> {
        self.book.get_role(&self.talker.role_id).unwrap()
    }
}

pub struct Noun<'a> {
    book: &'a Book,
    room: &'a RoomEntry,
    noun: &'a NounEntry,
    room_id: RoomId,
    noun_id: NounId,
}

impl<'a> Noun<'a> {
    #[expect(dead_code)]
    pub fn desc(&self) -> Option<&str> {
        self.noun.desc.as_deref()
    }

    #[expect(dead_code)]
    pub fn room(&self) -> Room<'a> {
        Room {
            parent: self.book,
            id: self.room_id,
            entry: self.room,
        }
    }

    #[expect(dead_code)]
    pub fn conversations(&self) -> impl Iterator<Item = Conversation> {
        self.noun.conversations.iter().map(|(k, v)| Conversation {
            book: self.book,
            room: self.room,
            noun: self.noun,
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
    pub fn name(&self) -> &str {
        &self.entry.name
    }

    /// Get an iterator over all the nouns in this room.
    #[expect(dead_code)]
    pub fn nouns(&self) -> impl Iterator<Item = Noun> {
        self.entry.nouns.iter().map(|(k, v)| Noun {
            book: self.parent,
            room: self.entry,
            room_id: self.id,
            noun_id: *k,
            noun: v,
        })
    }

    /// Get an iterator over all the conditions in this room.
    #[expect(dead_code)]
    pub fn conditions(&self) -> impl Iterator<Item = Condition> {
        self.entry.conditions.iter().map(|(k, v)| Condition {
            book: self.parent,
            room: self.entry,
            room_id: self.id,
            id: *k,
            condition: v,
        })
    }
}

pub struct Role<'a> {
    #[expect(dead_code)]
    parent: &'a Book,
    #[expect(dead_code)]
    id: &'a RoleId,
    entry: &'a RoleEntry,
}

impl<'a> Role<'a> {
    /// Get the full name of the role.
    #[expect(dead_code)]
    pub fn name(&self) -> &str {
        &self.entry.name
    }

    /// Get the short name of the role.
    #[expect(dead_code)]
    pub fn short_name(&self) -> &str {
        &self.entry.short_name
    }
}

pub struct Book {
    roles: BTreeMap<RoleId, RoleEntry>,
    talkers: BTreeMap<TalkerId, TalkerEntry>,
    verbs: BTreeMap<VerbId, VerbEntry>,
    rooms: BTreeMap<RoomId, RoomEntry>,
}

impl Book {
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
    pub fn roles(&self) -> impl Iterator<Item = Role> {
        self.roles.iter().map(|(k, v)| Role {
            parent: self,
            id: k,
            entry: v,
        })
    }

    pub fn get_role(&self, id: &RoleId) -> Option<Role> {
        self.roles.get_key_value(id).map(|(k, entry)| Role {
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

    pub fn get_talker(&self, id: TalkerId) -> Option<Talker> {
        self.talkers.get(&id).map(|entry| Talker {
            book: self,
            id,
            talker: entry,
        })
    }
}
