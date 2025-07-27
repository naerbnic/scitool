use std::collections::{BTreeMap, btree_map};

use itertools::Itertools;

use scidev_common::{
    ConversationKey, RawConditionId, RawNounId, RawRoomId, RawSequenceId, RawVerbId,
};
use scidev_resources::types::msg::{MessageId, MessageRecord};
use scidev_utils::validation::{IteratorExt as _, MultiValidator, ValidationError};

use super::{
    Book,
    config::{self, BookConfig},
    ids::{RawRoleId, RawTalkerId},
};

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct BuildError(Box<dyn std::error::Error + Send + Sync>);

impl BuildError {
    #[must_use]
    pub fn from_anyhow(err: anyhow::Error) -> Self {
        BuildError(err.into())
    }
}

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

/// Convert an iterator of pairs into a `BTreeMap`,
fn group_pairs<I, K, V>(pairs: I) -> BuildResult<BTreeMap<K, V>>
where
    I: IntoIterator<Item = (K, V)>,
    K: Ord + std::fmt::Debug,
{
    let mut map = BTreeMap::new();
    let mut dup_keys = Vec::new();
    for (key, value) in pairs {
        let key_fmt = format!("{key:?}");
        if map.insert(key, value).is_some() {
            dup_keys.push(format!("{key_fmt:?}"));
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

fn map_result_values<K, S, T, F>(source: &BTreeMap<K, S>, f: F) -> BuildResult<BTreeMap<K, T>>
where
    K: Ord + Clone,
    F: Fn(&S) -> BuildResult<T>,
{
    source.iter().map(|(k, v)| Ok((k.clone(), f(v)?))).collect()
}

fn map_values<'map, K, S, T, F>(source: &'map BTreeMap<K, S>, f: F) -> BTreeMap<K, T>
where
    K: Ord + Clone,
    F: Fn(&S) -> T,
    T: 'map,
{
    source.iter().map(|(k, v)| (k.clone(), f(v))).collect()
}

fn filter_map_values<K, S, T, F>(source: &BTreeMap<K, S>, body: F) -> BuildResult<BTreeMap<K, T>>
where
    K: Ord + Clone,
    F: Fn(&S) -> BuildResult<Option<T>>,
{
    source
        .iter()
        .filter_map(|(k, v)| match body(v) {
            Ok(Some(v)) => Some(Ok((k.clone(), v))),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        })
        .collect()
}

#[derive(Debug, Clone)]
pub(super) struct MessageEntry {
    talker: RawTalkerId,
    text: String,
}
impl MessageEntry {
    fn build(&self, ctxt: &BookBuilder) -> Result<super::LineEntry, BuildError> {
        Ok(super::LineEntry {
            text: self.text.parse().map_err(BuildError::from_anyhow)?,
            role: ctxt
                .talkers
                .get(&self.talker)
                .ok_or_else(|| {
                    BuildError::from_anyhow(anyhow::anyhow!("Unknown talker: {:?}", self.talker))
                })?
                .role
                .clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct Conversation(BTreeMap<RawSequenceId, MessageEntry>);

impl Conversation {
    pub(super) fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub(super) fn add_message(
        &mut self,
        message: MessageId,
        record: &MessageRecord,
    ) -> BuildResult<()> {
        match self.0.entry(RawSequenceId::new(message.sequence())) {
            btree_map::Entry::Vacant(vac) => {
                vac.insert(MessageEntry {
                    talker: RawTalkerId::new(record.talker()),
                    text: record.text().to_string(),
                });
                Ok(())
            }
            btree_map::Entry::Occupied(_) => Err("Sequence Collision".to_string().into()),
        }
    }

    fn build(&self, ctxt: &BookBuilder) -> BuildResult<super::ConversationEntry> {
        Ok(super::ConversationEntry {
            lines: map_result_values(&self.0, |v| v.build(ctxt))?,
        })
    }
}

pub(super) struct RoleEntry {
    name: String,
    short_name: String,
}

impl RoleEntry {
    fn build(&self) -> super::RoleEntry {
        super::RoleEntry {
            name: self.name.clone(),
            short_name: self.short_name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct TalkerEntry {
    role: RawRoleId,
}

impl TalkerEntry {
    fn validate(&self, ctxt: &BookBuilder) -> ValidateResult {
        if !ctxt.contains_role(&self.role) {
            return Err(format!("Talker references unknown role: {:?}", self.role).into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(super) struct VerbEntry {
    name: String,
}

impl VerbEntry {
    fn build(&self) -> super::VerbEntry {
        super::VerbEntry {
            name: self.name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ConditionEntry {
    desc: Option<String>,
}

impl ConditionEntry {
    pub(super) fn desc(&self) -> Option<&str> {
        self.desc.as_deref()
    }
}

impl ConditionEntry {
    fn build(&self) -> super::ConditionEntry {
        super::ConditionEntry {
            builder: self.clone(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct NounEntry {
    desc: Option<String>,
    is_cutscene: bool,
    conversation_set: BTreeMap<ConversationKey, Conversation>,
    hidden: bool,
}

impl NounEntry {
    pub(super) fn with_config(noun_entry: config::NounEntry) -> Self {
        Self {
            desc: Some(noun_entry.desc),
            is_cutscene: noun_entry.is_cutscene,
            conversation_set: BTreeMap::new(),
            hidden: noun_entry.hidden,
        }
    }

    fn add_message(&mut self, message: MessageId, record: &MessageRecord) -> BuildResult<()> {
        let key = ConversationKey::new(
            RawVerbId::new(message.verb()),
            RawConditionId::new(message.condition()),
        );

        self.conversation_set
            .entry(key)
            .or_insert_with(Conversation::new)
            .add_message(message, record)
    }

    fn validate(&self, _ctxt: &BookBuilder) -> ValidateResult {
        let mut validator = MultiValidator::new();
        if self.is_cutscene {
            match self.conversation_set.iter().exactly_one() {
                Ok((key, _)) => {
                    if key.verb() != RawVerbId::new(0) || key.condition() != RawConditionId::new(0)
                    {
                        validator.with_err(ValidationError::from(format!(
                            "Cutscene noun must have exactly one conversation with verb 0 and condition 0. Found: {key:?}"
                        )));
                    }
                }
                Err(conversations) => {
                    validator.with_err(ValidationError::from(format!(
                        "Cutscene noun must have exactly one conversation. Found: {}",
                        conversations.count()
                    )));
                }
            }
        }
        validator.build()?;
        Ok(())
    }

    fn build(&self, ctxt: &BookBuilder) -> Result<super::NounEntry, BuildError> {
        Ok(super::NounEntry {
            desc: self.desc.clone(),
            is_cutscene: self.is_cutscene,
            conversations: map_result_values(&self.conversation_set, |v| v.build(ctxt))?,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct RoomEntry {
    name: Option<String>,
    conditions: BTreeMap<RawConditionId, ConditionEntry>,
    nouns: BTreeMap<RawNounId, NounEntry>,
    hidden: bool,
}

impl RoomEntry {
    fn validate(&self, ctxt: &BookBuilder) -> ValidateResult {
        MultiValidator::new()
            .validate_ctxt("nouns", || {
                self.nouns.iter().validate_all_values(|e| e.validate(ctxt))
            })
            .build()?;
        Ok(())
    }

    fn build(&self, ctxt: &BookBuilder) -> BuildResult<super::RoomEntry> {
        Ok(super::RoomEntry {
            name: self.name.clone(),
            conditions: map_values(&self.conditions, ConditionEntry::build),
            nouns: filter_map_values(&self.nouns, |v| {
                Ok(if v.hidden { None } else { Some(v.build(ctxt)?) })
            })?,
        })
    }
}

impl RoomEntry {
    fn from_config(room_config: config::RoomEntry) -> BuildResult<Self> {
        Ok(Self {
            name: Some(room_config.name),
            conditions: group_pairs(room_config.conditions.into_iter().map(|condition| {
                (
                    condition.id,
                    ConditionEntry {
                        desc: Some(condition.desc),
                    },
                )
            }))?,
            nouns: group_pairs(
                room_config
                    .nouns
                    .into_iter()
                    .map(|noun| (noun.id, NounEntry::with_config(noun))),
            )?,
            hidden: room_config.hidden,
        })
    }

    fn add_message(&mut self, message: MessageId, record: &MessageRecord) -> BuildResult<()> {
        let condition_id = RawConditionId::new(message.condition());
        if let btree_map::Entry::Vacant(vac) = self.conditions.entry(condition_id) {
            vac.insert(ConditionEntry { desc: None });
        }
        self.nouns
            .entry(RawNounId::new(message.noun()))
            .or_default()
            .add_message(message, record)
    }
}

pub struct BookBuilder {
    project_name: String,
    roles: BTreeMap<RawRoleId, RoleEntry>,
    talkers: BTreeMap<RawTalkerId, TalkerEntry>,
    verbs: BTreeMap<RawVerbId, VerbEntry>,
    rooms: BTreeMap<RawRoomId, RoomEntry>,
}

impl BookBuilder {
    pub fn new(config: BookConfig) -> BuildResult<Self> {
        let builder = Self {
            project_name: config.project_name.clone(),
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
                    .map(|room| Ok((room.id, RoomEntry::from_config(room)?))),
            )?,
        };

        Ok(builder)
    }

    pub fn add_message(
        &mut self,
        room: u16,
        message: MessageId,
        record: &MessageRecord,
    ) -> BuildResult<&mut Self> {
        self.rooms
            .entry(RawRoomId::new(room))
            .or_default()
            .add_message(message, record)?;
        Ok(self)
    }

    pub fn build(self) -> BuildResult<Book> {
        self.validate()?;
        Ok(Book {
            project_name: self.project_name.clone(),
            roles: map_values(&self.roles, RoleEntry::build),
            verbs: map_values(&self.verbs, VerbEntry::build),
            rooms: filter_map_values(&self.rooms, |v| {
                Ok(if v.hidden {
                    None
                } else {
                    Some(v.build(&self)?)
                })
            })?,
        })
    }
}

/// Validation helpers. Internal
impl BookBuilder {
    fn validate(&self) -> ValidateResult {
        MultiValidator::new()
            .validate_ctxt("talkers", || {
                self.talkers
                    .iter()
                    .validate_all_values(|e| e.validate(self))
            })
            .validate_ctxt("rooms", || {
                self.rooms.iter().validate_all_values(|e| e.validate(self))
            })
            .build()?;
        Ok(())
    }

    fn contains_role(&self, role_id: &RawRoleId) -> bool {
        self.roles.contains_key(role_id)
    }
}
