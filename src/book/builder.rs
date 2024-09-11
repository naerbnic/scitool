use std::collections::{btree_map, BTreeMap};

use crate::{
    res::msg::{MessageId, MessageRecord},
    util::validation::{IteratorExt as _, MultiValidator, ValidationError},
};

use super::{
    config::{self, BookConfig},
    CastId, ConditionId, NounId, RoomId, SequenceId, TalkerId, VerbId,
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

pub(super) struct MessageEntry {
    #[expect(dead_code)]
    talker: TalkerId,
    #[expect(dead_code)]
    text: String,
}

/// A key for a conversation in a noun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConversationKey {
    verb: VerbId,
    condition: ConditionId,
}

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
}

pub(super) struct CastEntry {
    #[expect(dead_code)]
    name: String,
    #[expect(dead_code)]
    short_name: String,
}

impl CastEntry {
    fn validate(&self, _ctxt: &BookBuilder) -> BuildResult<()> {
        Ok(())
    }
}

pub(super) struct TalkerEntry {
    cast: CastId,
}

impl TalkerEntry {
    fn validate(&self, ctxt: &BookBuilder) -> ValidateResult {
        if !ctxt.contains_cast(&self.cast) {
            return Err(format!("Talker references unknown cast member: {:?}", self.cast).into());
        }
        Ok(())
    }
}

pub(super) struct VerbEntry {
    #[expect(dead_code)]
    name: String,
}

impl VerbEntry {
    fn validate(&self, _ctxt: &BookBuilder) -> ValidateResult {
        Ok(())
    }
}

pub(super) struct ConditionEntry {
    #[expect(dead_code)]
    desc: String,
}

impl ConditionEntry {
    fn validate(&self, _ctxt: &BookBuilder) -> ValidateResult {
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct NounEntry {
    #[expect(dead_code)]
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
}

pub(super) struct RoomEntry {
    #[expect(dead_code)]
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
    cast: BTreeMap<CastId, CastEntry>,
    talkers: BTreeMap<TalkerId, TalkerEntry>,
    verbs: BTreeMap<VerbId, VerbEntry>,
    rooms: BTreeMap<RoomId, RoomEntry>,
}

impl BookBuilder {
    pub fn new(config: BookConfig) -> BuildResult<Self> {
        let builder = Self {
            cast: group_pairs(config.cast.into_iter().map(|(k, v)| {
                (
                    k,
                    CastEntry {
                        name: v.name,
                        short_name: v.short_name,
                    },
                )
            }))?,
            talkers: group_pairs(
                config
                    .talkers
                    .into_iter()
                    .map(|talker| (talker.id, TalkerEntry { cast: talker.cast })),
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
            cast: self.cast,
            talkers: self.talkers,
            verbs: self.verbs,
            rooms: self.rooms,
        })
    }
}

/// Validation helpers. Internal
impl BookBuilder {
    fn validate(&self) -> ValidateResult {
        MultiValidator::new()
            .validate_ctxt("cast", &self.cast, |cast| {
                cast.iter().validate_all_values(|e| e.validate(self))
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

    fn contains_cast(&self, cast_id: &CastId) -> bool {
        self.cast.contains_key(cast_id)
    }
}
