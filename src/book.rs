//! Code to manage the organization and generation of VO scripts,
//! (referred to as "books" to disambguate from script resources).

use std::collections::BTreeMap;

use builder::ConversationKey;
use serde::{Deserialize, Serialize};

pub mod builder;
pub mod config;

// Raw IDs.
//
// There are the internal IDs used to reference different entities in the book.
// They are copyable, but only reference a single literal value from the SCI message
// file. They are used to construct the public IDs that are used to navigate the book.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
struct RawRoomId(u16);

impl From<u16> for RawRoomId {
    fn from(value: u16) -> Self {
        RawRoomId(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
struct RawNounId(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
struct RawVerbId(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
struct RawConditionId(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
struct RawSequenceId(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
struct RawTalkerId(u8);

// Book Specific IDs.

/// An identifier for a role.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
struct RawRoleId(String);

// Public IDs.
//
// These uniquely identify different entities in the book. They are frequently
// composite ids, in order to navigate to the correct entity in the book.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoomId(RawRoomId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VerbId(RawVerbId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoleId(RawRoleId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NounId(RoomId, RawNounId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TalkerId(RawTalkerId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConditionId(RoomId, RawConditionId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConversationId(NounId, ConversationKey);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineId(ConversationId, RawSequenceId);

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
    talker: RawTalkerId,
}

struct ConversationEntry {
    lines: BTreeMap<RawSequenceId, LineEntry>,
}

struct NounEntry {
    desc: Option<String>,
    conversations: BTreeMap<ConversationKey, ConversationEntry>,
}

struct RoomEntry {
    name: Option<String>,
    conditions: BTreeMap<RawConditionId, ConditionEntry>,
    nouns: BTreeMap<RawNounId, NounEntry>,
}

struct RoleEntry {
    name: String,
    short_name: String,
}

struct TalkerEntry {
    role_id: RawRoleId,
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
    id: LineId,
}

impl<'a> Line<'a> {
    #[expect(dead_code)]
    pub fn id(&self) -> LineId {
        self.id.clone()
    }

    #[expect(dead_code)]
    pub fn text(&self) -> &str {
        &self.line.text
    }

    pub fn talker(&self) -> Talker<'a> {
        self.book.get_talker(TalkerId(self.line.talker)).unwrap()
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
            id: self.id.0,
        }
    }
}

pub struct Conversation<'a> {
    book: &'a Book,
    room: &'a RoomEntry,
    noun: &'a NounEntry,
    conversation: &'a ConversationEntry,
    id: ConversationId,
}

impl<'a> Conversation<'a> {
    #[expect(dead_code)]
    pub fn id(&self) -> ConversationId {
        self.id
    }

    #[expect(dead_code)]
    pub fn lines(&self) -> impl Iterator<Item = Line> {
        self.conversation.lines.iter().map(|(k, v)| Line {
            book: self.book,
            room: self.room,
            noun: self.noun,
            conversation: self.conversation,
            line: v,
            id: LineId(self.id, *k),
        })
    }

    /// Get the noun this conversation is part of.
    #[expect(dead_code)]
    pub fn noun(&self) -> Noun<'a> {
        Noun {
            book: self.book,
            room: self.room,
            noun: self.noun,
            id: self.id.0,
        }
    }

    /// Get the verb used for this conversation (if it exists).
    #[expect(dead_code)]
    pub fn verb(&self) -> Option<Verb<'a>> {
        if self.id.1.verb() == RawVerbId(0) {
            return None;
        }
        Some(self.book.get_verb(VerbId(self.id.1.verb())).unwrap())
    }

    /// Get the condition needed for this conversation (if it exists).
    #[expect(dead_code)]
    pub fn condition(&self) -> Option<Condition<'a>> {
        if self.id.1.condition() == RawConditionId(0) {
            return None;
        }
        Some(Condition {
            book: self.book,
            room: self.room,
            id: ConditionId(self.id.0 .0, self.id.1.condition()),
            condition: self.room.conditions.get(&self.id.1.condition()).unwrap(),
        })
    }
}

pub struct Condition<'a> {
    book: &'a Book,
    room: &'a RoomEntry,
    condition: &'a ConditionEntry,
    id: ConditionId,
}

impl<'a> Condition<'a> {
    #[expect(dead_code)]
    pub fn id(&self) -> ConditionId {
        self.id
    }

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
            id: self.id.0,
            entry: self.room,
        }
    }
}

pub struct Verb<'a> {
    #[expect(dead_code)]
    book: &'a Book,
    verb: &'a VerbEntry,
    id: RawVerbId,
}

impl<'a> Verb<'a> {
    #[expect(dead_code)]
    pub fn id(&self) -> VerbId {
        VerbId(self.id)
    }

    #[expect(dead_code)]
    pub fn name(&self) -> &str {
        &self.verb.name
    }
}

pub struct Talker<'a> {
    book: &'a Book,
    talker: &'a TalkerEntry,
    id: RawTalkerId,
}

impl<'a> Talker<'a> {
    #[expect(dead_code)]
    pub fn id(&self) -> TalkerId {
        TalkerId(self.id)
    }

    pub fn role(&self) -> Role<'a> {
        self.book
            .get_role(&RoleId(self.talker.role_id.clone()))
            .unwrap()
    }
}

pub struct Noun<'a> {
    book: &'a Book,
    room: &'a RoomEntry,
    noun: &'a NounEntry,
    id: NounId,
}

impl<'a> Noun<'a> {
    #[expect(dead_code)]
    pub fn id(&self) -> NounId {
        self.id
    }

    #[expect(dead_code)]
    pub fn desc(&self) -> Option<&str> {
        self.noun.desc.as_deref()
    }

    #[expect(dead_code)]
    pub fn room(&self) -> Room<'a> {
        Room {
            parent: self.book,
            id: self.id.0,
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
            id: ConversationId(self.id, *k),
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
    pub fn id(&self) -> RoomId {
        self.id
    }

    pub fn name(&self) -> &str {
        self.entry.name.as_deref().unwrap_or("*NO NAME*")
    }

    /// Get an iterator over all the nouns in this room.
    #[expect(dead_code)]
    pub fn nouns(&self) -> impl Iterator<Item = Noun> {
        self.entry.nouns.iter().map(|(k, v)| Noun {
            book: self.parent,
            room: self.entry,
            noun: v,
            id: NounId(self.id, *k),
        })
    }

    /// Get an iterator over all the conditions in this room.
    #[expect(dead_code)]
    pub fn conditions(&self) -> impl Iterator<Item = Condition> {
        self.entry.conditions.iter().map(|(k, v)| Condition {
            book: self.parent,
            room: self.entry,
            id: ConditionId(self.id, *k),
            condition: v,
        })
    }
}

pub struct Role<'a> {
    #[expect(dead_code)]
    parent: &'a Book,
    id: &'a RawRoleId,
    entry: &'a RoleEntry,
}

impl<'a> Role<'a> {
    #[expect(dead_code)]
    pub fn id(&self) -> RoleId {
        RoleId(self.id.clone())
    }

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
    roles: BTreeMap<RawRoleId, RoleEntry>,
    talkers: BTreeMap<RawTalkerId, TalkerEntry>,
    verbs: BTreeMap<RawVerbId, VerbEntry>,
    rooms: BTreeMap<RawRoomId, RoomEntry>,
}

/// Public methods for the book.
impl Book {
    pub fn rooms(&self) -> impl Iterator<Item = Room> {
        self.rooms.iter().map(|(k, v)| Room {
            parent: self,
            id: RoomId(*k),
            entry: v,
        })
    }

    #[expect(dead_code)]
    pub fn get_room(&self, id: RoomId) -> Option<Room> {
        self.rooms.get(&id.0).map(|entry| Room {
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
        self.roles.get_key_value(&id.0).map(|(k, entry)| Role {
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
        self.verbs.get(&id.0).map(|entry| Verb {
            book: self,
            id: id.0,
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
        self.talkers.get(&id.0).map(|entry| Talker {
            book: self,
            id: id.0,
            talker: entry,
        })
    }
}
