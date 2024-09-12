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

#[derive(Clone)]
pub struct Line<'a> {
    conversation: Conversation<'a>,
    line: &'a LineEntry,
    id: RawSequenceId,
}

impl<'a> Line<'a> {
    #[expect(dead_code)]
    pub fn id(&self) -> LineId {
        LineId(self.conversation.id(), self.id)
    }

    #[expect(dead_code)]
    pub fn text(&self) -> &str {
        &self.line.text
    }

    pub fn talker(&self) -> Talker<'a> {
        self.book().get_talker(TalkerId(self.line.talker)).unwrap()
    }

    #[expect(dead_code)]
    pub fn role(&self) -> Role<'a> {
        self.talker().role()
    }

    #[expect(dead_code)]
    pub fn conversation(&self) -> Conversation<'a> {
        self.conversation.clone()
    }

    fn book(&self) -> &'a Book {
        self.conversation.book()
    }
}

#[derive(Clone)]
pub struct Conversation<'a> {
    noun: Noun<'a>,
    conversation: &'a ConversationEntry,
    id: ConversationKey,
}

impl<'a> Conversation<'a> {
    pub fn id(&self) -> ConversationId {
        ConversationId(self.noun.id(), self.id)
    }

    #[expect(dead_code)]
    pub fn lines(&self) -> impl Iterator<Item = Line> {
        self.conversation.lines.iter().map(|(k, v)| Line {
            conversation: self.clone(),
            line: v,
            id: *k,
        })
    }

    /// Get the noun this conversation is part of.
    pub fn noun(&self) -> Noun<'a> {
        self.noun.clone()
    }

    /// Get the verb used for this conversation (if it exists).
    #[expect(dead_code)]
    pub fn verb(&self) -> Option<Verb<'a>> {
        if self.id.verb() == RawVerbId(0) {
            return None;
        }
        Some(self.book().get_verb(VerbId(self.id.verb())).unwrap())
    }

    /// Get the condition needed for this conversation (if it exists).
    #[expect(dead_code)]
    pub fn condition(&self) -> Option<Condition<'a>> {
        if self.id.condition() == RawConditionId(0) {
            return None;
        }
        Some(
            self.noun()
                .room()
                .get_condition_inner(self.id.condition())
                .expect("Condition has already been cleared"),
        )
    }

    fn get_line_inner(&self, id: RawSequenceId) -> Option<Line<'a>> {
        self.conversation.lines.get(&id).map(|entry| Line {
            conversation: self.clone(),
            line: entry,
            id,
        })
    }

    fn book(&self) -> &'a Book {
        self.noun.book()
    }
}

#[derive(Clone)]
pub struct Condition<'a> {
    room: Room<'a>,
    condition: &'a ConditionEntry,
    id: RawConditionId,
}

impl<'a> Condition<'a> {
    #[expect(dead_code)]
    pub fn id(&self) -> ConditionId {
        ConditionId(self.room.id, self.id)
    }

    /// Get the description of this condition (if specified).
    #[expect(dead_code)]
    pub fn desc(&self) -> Option<&str> {
        self.condition.builder.as_ref().map(|b| b.desc())
    }

    /// Get the room this condition is part of.
    #[expect(dead_code)]
    pub fn room(&self) -> Room<'a> {
        self.room.clone()
    }

    #[expect(dead_code)]
    fn book(&self) -> &'a Book {
        self.room.book()
    }
}

#[derive(Clone)]
pub struct Verb<'a> {
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

    #[expect(dead_code)]
    fn book(&self) -> &Book {
        self.book
    }
}

#[derive(Clone)]
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

    #[expect(dead_code)]
    fn book(&self) -> &Book {
        self.book
    }
}

#[derive(Clone)]
pub struct Noun<'a> {
    room: Room<'a>,
    noun: &'a NounEntry,
    id: RawNounId,
}

impl<'a> Noun<'a> {
    pub fn id(&self) -> NounId {
        NounId(self.room.id, self.id)
    }

    #[expect(dead_code)]
    pub fn desc(&self) -> Option<&str> {
        self.noun.desc.as_deref()
    }

    pub fn room(&self) -> Room<'a> {
        self.room.clone()
    }

    #[expect(dead_code)]
    pub fn conversations(&self) -> impl Iterator<Item = Conversation> {
        self.noun.conversations.iter().map(|(k, v)| Conversation {
            noun: self.clone(),
            conversation: v,
            id: *k,
        })
    }

    fn get_conversation_inner(&self, id: ConversationKey) -> Option<Conversation<'a>> {
        self.noun
            .conversations
            .get(&id)
            .map(|conversation| Conversation {
                noun: self.clone(),
                conversation,
                id,
            })
    }

    fn book(&self) -> &'a Book {
        self.room.book()
    }
}

#[derive(Clone)]
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
            room: self.clone(),
            noun: v,
            id: *k,
        })
    }

    /// Get an iterator over all the conditions in this room.
    #[expect(dead_code)]
    pub fn conditions(&self) -> impl Iterator<Item = Condition> {
        self.entry.conditions.iter().map(|(k, v)| Condition {
            room: self.clone(),
            id: *k,
            condition: v,
        })
    }

    fn get_condition_inner(&self, id: RawConditionId) -> Option<Condition<'a>> {
        self.entry.conditions.get(&id).map(|entry| Condition {
            room: self.clone(),
            id,
            condition: entry,
        })
    }

    fn get_noun_inner(&self, id: RawNounId) -> Option<Noun<'a>> {
        self.entry.nouns.get(&id).map(|entry| Noun {
            room: self.clone(),
            id,
            noun: entry,
        })
    }

    fn book(&self) -> &'a Book {
        self.parent
    }
}

#[derive(Clone)]
pub struct Role<'a> {
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

    #[expect(dead_code)]
    fn book(&self) -> &Book {
        self.parent
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
    pub fn roles(&self) -> impl Iterator<Item = Role> {
        self.roles.iter().map(|(k, v)| Role {
            parent: self,
            id: k,
            entry: v,
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

    pub fn get_role(&self, id: &RoleId) -> Option<Role> {
        self.roles.get_key_value(&id.0).map(|(k, entry)| Role {
            parent: self,
            id: k,
            entry,
        })
    }

    pub fn get_verb(&self, id: VerbId) -> Option<Verb> {
        self.verbs.get(&id.0).map(|entry| Verb {
            book: self,
            id: id.0,
            verb: entry,
        })
    }

    pub fn get_room(&self, id: RoomId) -> Option<Room> {
        self.rooms.get(&id.0).map(|entry| Room {
            parent: self,
            id,
            entry,
        })
    }

    #[expect(dead_code)]
    pub fn get_condition(&self, id: ConditionId) -> Option<Condition> {
        self.get_room(id.0)
            .and_then(|room| room.get_condition_inner(id.1))
    }

    pub fn get_noun(&self, id: NounId) -> Option<Noun> {
        self.get_room(id.0)
            .and_then(|room| room.get_noun_inner(id.1))
    }

    pub fn get_conversation(&self, id: ConversationId) -> Option<Conversation> {
        self.get_noun(id.0)
            .and_then(|noun| noun.get_conversation_inner(id.1))
    }

    #[expect(dead_code)]
    pub fn get_line(&self, id: LineId) -> Option<Line> {
        self.get_conversation(id.0)
            .and_then(|conversation| conversation.get_line_inner(id.1))
    }
}
