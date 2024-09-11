use std::collections::{btree_map, BTreeMap};

use crate::{
    res::msg::{MessageId, MessageRecord},
    util::validation::{IteratorExt as _, MultiValidator, ValidationError},
};

use super::{
    config::{self, BookConfig},
    Book, RoleId, ConditionId, NounId, RoomId, SequenceId, TalkerId, VerbId,
};

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct BuildError(Box<dyn std::error::Error + Send + Sync>);

impl From<String> for BuildError {
    fn from(s: String) -> Self {
        BuildError(s.into())
    }
}

impl From<ValidationError> for BuildError {
    fn from(value: ValidationError) -> Self {
        BuildError(Box::new(value))
    }
}

pub type BuildResult<T> = Result<T, BuildError>;
pub type ValidateResult = Result<(), ValidationError>;

/// Convert an iterator of pairs into a BTreeMap,
fn group_pairs<I, K, V>(pairs: I) -> BuildResult<BTreeMap<K, V>>
where
    I: IntoIterator<Item = (K, V)>,
    K: Ord + std::fmt::Debug,
{
    let mut map = BTreeMap::new();
    let mut dup_keys = Vec::new();
    for (key, value) in pairs {
        let key_fmt = format!("{:?}", key);
        if map.insert(key, value).is_some() {
            dup_keys.push(format!("{:?}", key_fmt));
        }
    }
    if dup_keys.is_empty() {
        Ok(map)
    } else {
        Err(format!("Found duplicate keys: {}", dup_keys.join(", ")).into())
    }
}

fn group_pairs_with_errors<I, K, V>(pairs: I) -> BuildResult<BTreeMap<K, V>>
where
    I: IntoIterator<Item = Result<(K, V), BuildError>>,
    K: Ord + std::fmt::Debug,
{
    let resolved = pairs.into_iter().collect::<Result<Vec<_>, _>>()?;
    group_pairs(resolved)
}

fn map_values<K, S, T, F>(source: &BTreeMap<K, S>, f: F) -> BuildResult<BTreeMap<K, T>>
where
    K: Ord + Clone,
    F: Fn(&S) -> BuildResult<T>,
{
    source.iter().map(|(k, v)| Ok((k.clone(), f(v)?))).collect()
}

#[derive(Debug, Clone)]
pub(super) struct MessageEntry {
    talker: TalkerId,
    text: String,
}
impl MessageEntry {
    fn build(&self, _ctxt: &Conversation) -> Result<super::LineEntry, BuildError> {
        Ok(super::LineEntry {
            text: self.text.clone(),
            talker: self.talker,
        })
    }
}

/// A key for a conversation in a noun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConversationKey {
    verb: VerbId,
    condition: ConditionId,
}

impl ConversationKey {
    #[expect(dead_code)]
    pub fn new(verb: VerbId, condition: ConditionId) -> Self {
        Self { verb, condition }
    }

    pub fn verb(&self) -> VerbId {
        self.verb
    }

    pub fn condition(&self) -> ConditionId {
        self.condition
    }
}

#[derive(Debug, Clone)]
pub(super) struct Conversation(BTreeMap<SequenceId, MessageEntry>);

impl Conversation {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn add_message(&mut self, message: &MessageId, record: &MessageRecord) -> BuildResult<()> {
        match self.0.entry(SequenceId(message.sequence())) {
            btree_map::Entry::Vacant(vac) => {
                vac.insert(MessageEntry {
                    talker: TalkerId(record.talker()),
                    text: record.text().to_string(),
                });
                Ok(())
            }
            btree_map::Entry::Occupied(_) => Err("Sequence Collision".to_string().into()),
        }
    }

    fn build(&self, _ctxt: &BookBuilder) -> BuildResult<super::ConversationEntry> {
        Ok(super::ConversationEntry {
            lines: map_values(&self.0, |v| v.build(self))?,
        })
    }
}

pub(super) struct RoleEntry {
    name: String,
    short_name: String,
}

impl RoleEntry {
    fn validate(&self, _ctxt: &BookBuilder) -> BuildResult<()> {
        Ok(())
    }

    fn build(&self, ctxt: &BookBuilder) -> BuildResult<super::RoleEntry> {
        self.validate(ctxt)?;
        Ok(super::RoleEntry {
            name: self.name.clone(),
            short_name: self.short_name.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct TalkerEntry {
    role: RoleId,
}

impl TalkerEntry {
    fn validate(&self, ctxt: &BookBuilder) -> ValidateResult {
        if !ctxt.contains_role(&self.role) {
            return Err(format!("Talker references unknown role: {:?}", self.role).into());
        }
        Ok(())
    }

    fn build(&self, _arg: &BookBuilder) -> Result<super::TalkerEntry, BuildError> {
        Ok(super::TalkerEntry {
            role_id: self.role.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct VerbEntry {
    name: String,
}

impl VerbEntry {
    fn validate(&self, _ctxt: &BookBuilder) -> ValidateResult {
        Ok(())
    }

    fn build(&self, _arg: &BookBuilder) -> Result<super::VerbEntry, BuildError> {
        Ok(super::VerbEntry {
            name: self.name.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct ConditionEntry {
    desc: String,
}

impl ConditionEntry {
    pub fn desc(&self) -> &str {
        &self.desc
    }
}

impl ConditionEntry {
    fn validate(&self, _ctxt: &BookBuilder) -> ValidateResult {
        Ok(())
    }

    fn build(&self, _ctxt: &BookBuilder) -> Result<super::ConditionEntry, BuildError> {
        Ok(super::ConditionEntry {
            builder: Some(self.clone()),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct NounEntry {
    desc: Option<String>,
    conversation_set: BTreeMap<ConversationKey, Conversation>,
}

impl NounEntry {
    pub fn with_desc(desc: impl Into<String>) -> Self {
        Self {
            desc: Some(desc.into()),
            conversation_set: BTreeMap::new(),
        }
    }

    fn add_message(&mut self, message: &MessageId, record: &MessageRecord) -> BuildResult<()> {
        let key = ConversationKey {
            verb: VerbId(message.verb()),
            condition: ConditionId(message.condition()),
        };

        self.conversation_set
            .entry(key)
            .or_insert_with(Conversation::new)
            .add_message(message, record)
    }

    fn validate(&self, _ctxt: &BookBuilder) -> ValidateResult {
        Ok(())
    }

    fn build(&self, ctxt: &BookBuilder) -> Result<super::NounEntry, BuildError> {
        Ok(super::NounEntry {
            desc: self.desc.clone(),
            conversations: map_values(&self.conversation_set, |v| v.build(ctxt))?,
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct RoomEntry {
    name: String,
    conditions: BTreeMap<ConditionId, ConditionEntry>,
    nouns: BTreeMap<NounId, NounEntry>,
}

impl RoomEntry {
    fn validate(&self, ctxt: &BookBuilder) -> ValidateResult {
        MultiValidator::new()
            .validate_ctxt("conditions", &self.conditions, |conditions| {
                conditions.iter().validate_all_values(|e| e.validate(ctxt))
            })
            .validate_ctxt("conditions", &self.nouns, |nouns| {
                nouns.iter().validate_all_values(|e| e.validate(ctxt))
            })
            .build()?;
        Ok(())
    }

    fn build(&self, ctxt: &BookBuilder) -> BuildResult<super::RoomEntry> {
        Ok(super::RoomEntry {
            name: self.name.clone(),
            conditions: map_values(&self.conditions, |v| v.build(ctxt))?,
            nouns: map_values(&self.nouns, |v| v.build(ctxt))?,
        })
    }
}

impl RoomEntry {
    fn new(room_config: config::RoomEntry) -> BuildResult<Self> {
        Ok(Self {
            name: room_config.name,
            conditions: group_pairs(room_config.conditions.into_iter().map(|condition| {
                (
                    condition.id,
                    ConditionEntry {
                        desc: condition.desc,
                    },
                )
            }))?,
            nouns: group_pairs(
                room_config
                    .nouns
                    .into_iter()
                    .map(|noun| (noun.id, NounEntry::with_desc(noun.desc))),
            )?,
        })
    }

    fn add_message(&mut self, message: &MessageId, record: &MessageRecord) -> BuildResult<()> {
        self.nouns
            .entry(NounId(message.noun()))
            .or_default()
            .add_message(message, record)
    }
}

pub struct BookBuilder {
    roles: BTreeMap<RoleId, RoleEntry>,
    talkers: BTreeMap<TalkerId, TalkerEntry>,
    verbs: BTreeMap<VerbId, VerbEntry>,
    rooms: BTreeMap<RoomId, RoomEntry>,
}

impl BookBuilder {
    pub fn new(config: BookConfig) -> BuildResult<Self> {
        let builder = Self {
            roles: group_pairs(config.roles.into_iter().map(|(k, v)| {
                (
                    k,
                    RoleEntry {
                        name: v.name,
                        short_name: v.short_name,
                    },
                )
            }))?,
            talkers: group_pairs(
                config
                    .talkers
                    .into_iter()
                    .map(|talker| (talker.id, TalkerEntry { role: talker.role })),
            )?,
            verbs: group_pairs(
                config
                    .verbs
                    .into_iter()
                    .map(|verb| (verb.id, VerbEntry { name: verb.name })),
            )?,
            rooms: group_pairs_with_errors(
                config
                    .rooms
                    .into_iter()
                    .map(|room| Ok((room.id, RoomEntry::new(room)?))),
            )?,
        };

        Ok(builder)
    }

    pub fn add_message(
        &mut self,
        room: RoomId,
        message: &MessageId,
        record: &MessageRecord,
    ) -> BuildResult<&mut Self> {
        self.rooms
            .get_mut(&room)
            .ok_or_else(|| format!("Room not found: {:?}", room))?
            .add_message(message, record)?;
        Ok(self)
    }

    pub fn build(self) -> BuildResult<Book> {
        self.validate()?;
        Ok(Book {
            roles: map_values(&self.roles, |v| v.build(&self))?,
            talkers: map_values(&self.talkers, |v| v.build(&self))?,
            verbs: map_values(&self.verbs, |v| v.build(&self))?,
            rooms: map_values(&self.rooms, |v| v.build(&self))?,
        })
    }
}

/// Validation helpers. Internal
impl BookBuilder {
    fn validate(&self) -> ValidateResult {
        MultiValidator::new()
            .validate_ctxt("roles", &self.roles, |roles| {
                roles.iter().validate_all_values(|e| e.validate(self))
            })
            .validate_ctxt("talkers", &self.talkers, |talkers| {
                talkers.iter().validate_all_values(|e| e.validate(self))
            })
            .validate_ctxt("verbs", &self.verbs, |verbs| {
                verbs.iter().validate_all_values(|e| e.validate(self))
            })
            .validate_ctxt("rooms", &self.rooms, |rooms| {
                rooms.iter().validate_all_values(|e| e.validate(self))
            })
            .build()?;
        Ok(())
    }

    fn contains_role(&self, role_id: &RoleId) -> bool {
        self.roles.contains_key(role_id)
    }
}
