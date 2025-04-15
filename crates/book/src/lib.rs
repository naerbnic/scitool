//! Code to manage the organization and generation of VO scripts,
//! (referred to as "books" to disambguate from script resources).

use std::collections::BTreeMap;

use ids::{
    ConditionId, ConversationKey, RawConditionId, RawNounId, RawRoleId, RawRoomId, RawSequenceId,
    RawTalkerId, RawVerbId, TalkerId,
};

use sci_utils::validation::{MultiValidator, ValidationError};

pub mod builder;
pub mod config;
mod ids;
mod text;

pub use ids::{ConversationId, LineId, NounId, RoleId, RoomId, VerbId};
pub use text::{ColorControl, Control, FontControl, MessageSegment, MessageText};

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
    builder: builder::ConditionEntry,
}

struct LineEntry {
    text: MessageText,
    talker: RawTalkerId,
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
    parent: Conversation<'a>,
    raw_id: RawSequenceId,
    entry: &'a LineEntry,
}

impl<'a> Line<'a> {
    pub fn id(&self) -> LineId {
        LineId::from_conv_seq(self.parent.id(), self.raw_id)
    }

    pub fn text(&self) -> &MessageText {
        &self.entry.text
    }

    pub fn talker(&self) -> Talker<'a> {
        self.book()
            .get_talker(TalkerId::from_raw(self.entry.talker))
            .unwrap_or_else(|| {
                panic!(
                    "Talker not found: {:?} in line: {:?}",
                    self.entry.talker,
                    self.id()
                )
            })
    }

    pub fn role(&self) -> Role<'a> {
        self.talker().role()
    }

    pub fn conversation(&self) -> Conversation<'a> {
        self.parent.clone()
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
    pub fn noun(&self) -> Noun<'a> {
        self.parent.clone()
    }

    /// Get the verb used for this conversation (if it exists).
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
        for id in self.entry.lines.keys().map(|id| id.as_u8()) {
            if id != expected_next {
                validator.with_err(ValidationError::from(
                    format!("Skipped sequence ID {}, next {}", expected_next, id).to_string(),
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
    pub fn id(&self) -> ConditionId {
        ConditionId::from_room_raw(self.parent.id(), self.raw_id)
    }

    /// Get the description of this condition (if specified).
    pub fn desc(&self) -> Option<&str> {
        self.entry.builder.desc()
    }

    /// Get the room this condition is part of.
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
    pub fn id(&self) -> VerbId {
        VerbId::from_raw(self.raw_id)
    }

    pub fn name(&self) -> &str {
        &self.entry.name
    }

    #[expect(dead_code)]
    fn book(&self) -> &Book {
        self.parent
    }
}

#[derive(Clone)]
pub struct Talker<'a> {
    parent: &'a Book,
    raw_id: RawTalkerId,
    entry: &'a TalkerEntry,
}

impl<'a> Talker<'a> {
    pub fn id(&self) -> TalkerId {
        TalkerId::from_raw(self.raw_id)
    }

    pub fn role(&self) -> Role<'a> {
        self.parent
            .get_role(&RoleId::from_raw(self.entry.role_id.clone()))
            .unwrap()
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
    pub fn id(&self) -> NounId {
        NounId::from_room_raw(self.parent.id(), self.raw_id)
    }

    pub fn desc(&self) -> Option<&str> {
        self.entry.desc.as_deref()
    }

    pub fn is_cutscene(&self) -> bool {
        self.entry.is_cutscene
    }

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
    pub fn id(&self) -> RoomId {
        RoomId::from_raw(self.raw_id)
    }

    pub fn name(&self) -> &str {
        self.entry.name.as_deref().unwrap_or("*NO NAME*")
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
    pub fn id(&self) -> RoleId {
        RoleId::from_raw(self.raw_id.clone())
    }

    /// Get the full name of the role.
    pub fn name(&self) -> &str {
        &self.entry.name
    }

    /// Get the short name of the role.
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
    talkers: BTreeMap<RawTalkerId, TalkerEntry>,
    verbs: BTreeMap<RawVerbId, VerbEntry>,
    rooms: BTreeMap<RawRoomId, RoomEntry>,
}

/// Public methods for the book.
impl Book {
    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    pub fn rooms(&self) -> impl Iterator<Item = Room> {
        self.rooms.iter().map(|(&raw_id, entry)| Room {
            parent: self,
            raw_id,
            entry,
        })
    }

    pub fn roles(&self) -> impl Iterator<Item = Role> {
        self.roles.iter().map(|(raw_id, entry)| Role {
            parent: self,
            raw_id,
            entry,
        })
    }

    pub fn verbs(&self) -> impl Iterator<Item = Verb> {
        self.verbs.iter().map(|(&raw_id, entry)| Verb {
            parent: self,
            raw_id,
            entry,
        })
    }

    pub fn talkers(&self) -> impl Iterator<Item = Talker> {
        self.talkers.iter().map(|(k, v)| Talker {
            parent: self,
            raw_id: *k,
            entry: v,
        })
    }

    pub fn nouns(&self) -> impl Iterator<Item = Noun> {
        self.rooms().flat_map(|room| room.nouns())
    }

    pub fn conversations(&self) -> impl Iterator<Item = Conversation> + '_ {
        self.nouns().flat_map(|noun| noun.conversations())
    }

    pub fn lines(&self) -> impl Iterator<Item = Line> + '_ {
        self.conversations()
            .flat_map(|conversation| conversation.lines())
    }

    pub fn conditions(&self) -> impl Iterator<Item = Condition> + '_ {
        self.rooms().flat_map(|room| room.conditions())
    }

    pub fn get_talker(&self, id: TalkerId) -> Option<Talker> {
        self.talkers
            .get_key_value(&id.raw_id())
            .map(|(&raw_id, entry)| Talker {
                parent: self,
                raw_id,
                entry,
            })
    }

    pub fn get_role(&self, id: &RoleId) -> Option<Role> {
        self.roles
            .get_key_value(id.raw_id())
            .map(|(raw_id, entry)| Role {
                parent: self,
                raw_id,
                entry,
            })
    }

    pub fn get_verb(&self, id: VerbId) -> Option<Verb> {
        self.verbs
            .get_key_value(&id.raw_id())
            .map(|(&raw_id, entry)| Verb {
                parent: self,
                raw_id,
                entry,
            })
    }

    pub fn get_room(&self, id: RoomId) -> Option<Room> {
        self.rooms
            .get_key_value(&id.raw_id())
            .map(|(&raw_id, entry)| Room {
                parent: self,
                raw_id,
                entry,
            })
    }

    pub fn get_condition(&self, id: ConditionId) -> Option<Condition> {
        self.get_room(id.room_id())
            .and_then(|room| room.get_condition_inner(id.raw_id()))
    }

    pub fn get_noun(&self, id: NounId) -> Option<Noun> {
        self.get_room(id.room_id())
            .and_then(|room| room.get_noun_inner(id.raw_id()))
    }

    pub fn get_conversation(&self, id: ConversationId) -> Option<Conversation> {
        self.get_noun(id.noun_id())
            .and_then(|noun| noun.get_conversation_inner(id.conversation_key()))
    }

    pub fn get_line(&self, id: LineId) -> Option<Line> {
        self.get_conversation(id.conv_id())
            .and_then(|conversation| conversation.get_line_inner(id.raw_id()))
    }
}
