//! Code to manage the organization and generation of VO scripts,
//! (referred to as "books" to disambguate from script resources).

use std::collections::BTreeMap;

use ids::RawRoleId;

use scidev::{
    common::{
        ConditionId, ConversationId, ConversationKey, LineId, NounId, RawConditionId, RawNounId,
        RawRoomId, RawSequenceId, RawVerbId, RoomId,
    },
    utils::validation::{MultiValidator, ValidationError},
};

pub mod builder;
pub mod config;
pub mod file_format;
mod ids;
mod message_text;
pub mod rich_text;

pub use ids::{RoleId, VerbId};
pub use message_text::{ColorControl, Control, FontControl, MessageSegment, MessageText};

use crate::{ids::RawTalkerId, rich_text::RichText};

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
    desc: Option<String>,
}

struct LineEntry {
    text: RichText,
    talker: RawTalkerId,
    role: RawRoleId,
}

struct ConversationEntry {
    lines: BTreeMap<RawSequenceId, LineEntry>,
}

struct NounEntry {
    desc: Option<String>,
    is_cutscene: bool,
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
    parent: Conversation<'a>,
    raw_id: RawSequenceId,
    entry: &'a LineEntry,
}

impl<'a> Line<'a> {
    #[must_use]
    pub fn id(&self) -> LineId {
        LineId::from_conv_seq(self.parent.id(), self.raw_id)
    }

    #[must_use]
    pub fn text(&self) -> &RichText {
        &self.entry.text
    }

    #[must_use]
    pub fn role(&self) -> Role<'a> {
        self.book()
            .get_role(&RoleId::from_raw(self.entry.role.clone()))
            .unwrap_or_else(|| {
                panic!(
                    "Role not found: {:?} in line: {:?}",
                    self.entry.role,
                    self.id()
                )
            })
    }

    #[must_use]
    pub fn conversation(&self) -> Conversation<'a> {
        self.parent.clone()
    }

    #[must_use]
    pub fn talker_num(&self) -> u8 {
        self.entry.talker.as_u8()
    }

    fn book(&self) -> &'a Book {
        self.parent.book()
    }
}

#[derive(Clone)]
pub struct Conversation<'a> {
    parent: Noun<'a>,
    raw_id: ConversationKey,
    entry: &'a ConversationEntry,
}

impl<'a> Conversation<'a> {
    #[must_use]
    pub fn id(&self) -> ConversationId {
        ConversationId::from_noun_key(self.parent.id(), self.raw_id)
    }

    pub fn lines(&self) -> impl Iterator<Item = Line<'a>> + 'a + use<'a> {
        self.entry.lines.iter().map({
            let parent = self.clone();
            move |(&raw_id, entry)| Line {
                parent: parent.clone(),
                raw_id,
                entry,
            }
        })
    }

    /// Get the noun this conversation is part of.
    #[must_use]
    pub fn noun(&self) -> Noun<'a> {
        self.parent.clone()
    }

    /// Get the verb used for this conversation (if it exists).
    #[must_use]
    pub fn verb(&self) -> Option<Verb<'a>> {
        if self.raw_id.verb() == RawVerbId::new(0) {
            return None;
        }
        Some(
            self.book()
                .get_verb(VerbId::from_raw(self.raw_id.verb()))
                .unwrap_or_else(|| {
                    panic!(
                        "Verb not found: {:?} in conversation: {:?}",
                        self.raw_id.verb(),
                        self.id()
                    )
                }),
        )
    }

    /// Get the condition needed for this conversation (if it exists).
    #[must_use]
    pub fn condition(&self) -> Option<Condition<'a>> {
        if self.raw_id.condition() == RawConditionId::new(0) {
            return None;
        }
        Some(
            self.noun()
                .room()
                .get_condition_inner(self.raw_id.condition())
                .expect("Condition has already been cleared"),
        )
    }

    pub fn validate_complete(&self) -> Result<(), ValidationError> {
        let mut validator = MultiValidator::new();
        let mut expected_next = 1;
        for id in self.entry.lines.keys().map(RawSequenceId::as_u8) {
            if id != expected_next {
                validator.with_err(ValidationError::from(
                    format!("Skipped sequence ID {expected_next}, next {id}").to_string(),
                ));
            }
            expected_next = id + 1;
        }

        validator.build()
    }

    fn get_line_inner(&self, raw_id: RawSequenceId) -> Option<Line<'a>> {
        self.entry.lines.get(&raw_id).map(|entry| Line {
            parent: self.clone(),
            raw_id,
            entry,
        })
    }

    fn book(&self) -> &'a Book {
        self.parent.book()
    }
}

#[derive(Clone)]
pub struct Condition<'a> {
    parent: Room<'a>,
    raw_id: RawConditionId,
    entry: &'a ConditionEntry,
}

impl<'a> Condition<'a> {
    #[must_use]
    pub fn id(&self) -> ConditionId {
        ConditionId::from_room_raw(self.parent.id(), self.raw_id)
    }

    /// Get the description of this condition (if specified).
    #[must_use]
    pub fn desc(&self) -> Option<&str> {
        self.entry.desc.as_deref()
    }

    /// Get the room this condition is part of.
    #[must_use]
    pub fn room(&self) -> Room<'a> {
        self.parent.clone()
    }

    #[expect(dead_code)]
    fn book(&self) -> &'a Book {
        self.parent.book()
    }
}

#[derive(Clone)]
pub struct Verb<'a> {
    parent: &'a Book,
    raw_id: RawVerbId,
    entry: &'a VerbEntry,
}

impl Verb<'_> {
    #[must_use]
    pub fn id(&self) -> VerbId {
        VerbId::from_raw(self.raw_id)
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.entry.name
    }

    #[expect(dead_code)]
    fn book(&self) -> &Book {
        self.parent
    }
}

#[derive(Clone)]
pub struct Noun<'a> {
    parent: Room<'a>,
    raw_id: RawNounId,
    entry: &'a NounEntry,
}

impl<'a> Noun<'a> {
    #[must_use]
    pub fn id(&self) -> NounId {
        NounId::from_room_raw(self.parent.id(), self.raw_id)
    }

    #[must_use]
    pub fn desc(&self) -> Option<&str> {
        self.entry.desc.as_deref()
    }

    #[must_use]
    pub fn is_cutscene(&self) -> bool {
        self.entry.is_cutscene
    }

    #[must_use]
    pub fn room(&self) -> Room<'a> {
        self.parent.clone()
    }

    pub fn conversations(&self) -> impl Iterator<Item = Conversation<'a>> + use<'a> {
        self.entry.conversations.iter().map({
            let parent = self.clone();
            move |(&raw_id, entry)| Conversation {
                parent: parent.clone(),
                raw_id,
                entry,
            }
        })
    }

    fn get_conversation_inner(&self, raw_id: ConversationKey) -> Option<Conversation<'a>> {
        self.entry
            .conversations
            .get(&raw_id)
            .map(|entry| Conversation {
                parent: self.clone(),
                raw_id,
                entry,
            })
    }

    fn book(&self) -> &'a Book {
        self.parent.book()
    }
}

#[derive(Clone)]
pub struct Room<'a> {
    parent: &'a Book,
    raw_id: RawRoomId,
    entry: &'a RoomEntry,
}

impl<'a> Room<'a> {
    #[must_use]
    pub fn id(&self) -> RoomId {
        RoomId::from_raw(self.raw_id)
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.entry.name.as_deref()
    }

    /// Get an iterator over all the nouns in this room.
    pub fn nouns(&self) -> impl Iterator<Item = Noun<'a>> + 'a + use<'a> {
        self.entry.nouns.iter().map({
            let parent = self.clone();
            move |(&raw_id, entry)| Noun {
                parent: parent.clone(),
                raw_id,
                entry,
            }
        })
    }

    /// Get an iterator over all the conditions in this room.
    pub fn conditions(&self) -> impl Iterator<Item = Condition<'a>> + 'a + use<'a> {
        self.entry.conditions.iter().map({
            let parent = self.clone();
            move |(&raw_id, entry)| Condition {
                parent: parent.clone(),
                raw_id,
                entry,
            }
        })
    }

    fn get_condition_inner(&self, raw_id: RawConditionId) -> Option<Condition<'a>> {
        self.entry.conditions.get(&raw_id).map(|entry| Condition {
            parent: self.clone(),
            raw_id,
            entry,
        })
    }

    fn get_noun_inner(&self, raw_id: RawNounId) -> Option<Noun<'a>> {
        self.entry.nouns.get(&raw_id).map(|entry| Noun {
            parent: self.clone(),
            raw_id,
            entry,
        })
    }

    fn book(&self) -> &'a Book {
        self.parent
    }
}

#[derive(Clone)]
pub struct Role<'a> {
    parent: &'a Book,
    raw_id: &'a RawRoleId,
    entry: &'a RoleEntry,
}

impl Role<'_> {
    #[must_use]
    pub fn id(&self) -> RoleId {
        RoleId::from_raw(self.raw_id.clone())
    }

    /// Get the full name of the role.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.entry.name
    }

    /// Get the short name of the role.
    #[must_use]
    pub fn short_name(&self) -> &str {
        &self.entry.short_name
    }

    #[expect(dead_code)]
    fn book(&self) -> &Book {
        self.parent
    }
}

pub struct Book {
    project_name: String,
    roles: BTreeMap<RawRoleId, RoleEntry>,
    verbs: BTreeMap<RawVerbId, VerbEntry>,
    rooms: BTreeMap<RawRoomId, RoomEntry>,
}

/// Public methods for the book.
impl Book {
    #[must_use]
    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    pub fn rooms(&'_ self) -> impl Iterator<Item = Room<'_>> {
        self.rooms.iter().map(|(&raw_id, entry)| Room {
            parent: self,
            raw_id,
            entry,
        })
    }

    pub fn roles(&'_ self) -> impl Iterator<Item = Role<'_>> {
        self.roles.iter().map(|(raw_id, entry)| Role {
            parent: self,
            raw_id,
            entry,
        })
    }

    pub fn verbs(&'_ self) -> impl Iterator<Item = Verb<'_>> {
        self.verbs.iter().map(|(&raw_id, entry)| Verb {
            parent: self,
            raw_id,
            entry,
        })
    }

    pub fn nouns(&'_ self) -> impl Iterator<Item = Noun<'_>> {
        self.rooms().flat_map(|room| room.nouns())
    }

    pub fn conversations(&'_ self) -> impl Iterator<Item = Conversation<'_>> + '_ {
        self.nouns().flat_map(|noun| noun.conversations())
    }

    pub fn lines(&'_ self) -> impl Iterator<Item = Line<'_>> + '_ {
        self.conversations()
            .flat_map(|conversation| conversation.lines())
    }

    pub fn conditions(&'_ self) -> impl Iterator<Item = Condition<'_>> + '_ {
        self.rooms().flat_map(|room| room.conditions())
    }

    #[must_use]
    pub fn get_role(&'_ self, id: &RoleId) -> Option<Role<'_>> {
        self.roles
            .get_key_value(id.raw_id())
            .map(|(raw_id, entry)| Role {
                parent: self,
                raw_id,
                entry,
            })
    }

    #[must_use]
    pub fn get_verb(&'_ self, id: VerbId) -> Option<Verb<'_>> {
        self.verbs
            .get_key_value(&id.raw_id())
            .map(|(&raw_id, entry)| Verb {
                parent: self,
                raw_id,
                entry,
            })
    }

    #[must_use]
    pub fn get_room(&'_ self, id: RoomId) -> Option<Room<'_>> {
        self.rooms
            .get_key_value(&id.raw_id())
            .map(|(&raw_id, entry)| Room {
                parent: self,
                raw_id,
                entry,
            })
    }

    #[must_use]
    pub fn get_condition(&'_ self, id: ConditionId) -> Option<Condition<'_>> {
        self.get_room(id.room_id())
            .and_then(|room| room.get_condition_inner(id.raw_id()))
    }

    #[must_use]
    pub fn get_noun(&'_ self, id: NounId) -> Option<Noun<'_>> {
        self.get_room(id.room_id())
            .and_then(|room| room.get_noun_inner(id.raw_id()))
    }

    #[must_use]
    pub fn get_conversation(&'_ self, id: ConversationId) -> Option<Conversation<'_>> {
        self.get_noun(id.noun_id())
            .and_then(|noun| noun.get_conversation_inner(id.conversation_key()))
    }

    #[must_use]
    pub fn get_line(&'_ self, id: LineId) -> Option<Line<'_>> {
        self.get_conversation(id.conv_id())
            .and_then(|conversation| conversation.get_line_inner(id.raw_id()))
    }
}
