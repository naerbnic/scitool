use std::collections::BTreeMap;

use super::{
    config::{self, BookConfig},
    CastId, ConditionId, NounId, RoomId, TalkerId, VerbId,
};

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct BuildError(Box<dyn std::error::Error + Send + Sync>);

impl From<String> for BuildError {
    fn from(s: String) -> Self {
        BuildError(s.into())
    }
}

pub type BuildResult<T> = Result<T, BuildError>;

/// Convert an iterator of pairs into a BTreeMap,
fn group_pairs<I, K, V>(pairs: I) -> BuildResult<BTreeMap<K, V>>
where
    I: IntoIterator<Item = (K, V)>,
    K: Ord + std::fmt::Debug,
{
    let mut map = BTreeMap::new();
    let mut dup_keys = Vec::new();
    for (key, value) in pairs {
        #[allow(clippy::map_entry, reason = "Avoid needing to clone key")]
        if map.contains_key(&key) {
            dup_keys.push(format!("{:?}", key));
        } else {
            map.insert(key, value);
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

fn validate_all<'a, I, K, V, F>(entries: I, validator: F) -> BuildResult<()>
where
    K: 'a,
    V: 'a,
    I: IntoIterator<Item = (&'a K, &'a V)>,
    F: Fn(&V) -> BuildResult<()>,
{
    for (_key, entry) in entries {
        validator(entry)?
    }
    Ok(())
}

struct CastEntry {
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

struct TalkerEntry {
    cast: CastId,
}

impl TalkerEntry {
    fn validate(&self, ctxt: &BookBuilder) -> BuildResult<()> {
        if !ctxt.contains_cast(&self.cast) {
            return Err(format!("Talker references unknown cast member: {:?}", self.cast).into());
        }
        Ok(())
    }
}

struct VerbEntry {
    #[expect(dead_code)]
    name: String,
}

impl VerbEntry {
    fn validate(&self, _ctxt: &BookBuilder) -> BuildResult<()> {
        Ok(())
    }
}

struct ConditionEntry {
    #[expect(dead_code)]
    desc: String,
}

impl ConditionEntry {
    fn validate(&self, _ctxt: &BookBuilder) -> BuildResult<()> {
        Ok(())
    }
}

struct NounEntry {
    #[expect(dead_code)]
    desc: String,
}

impl NounEntry {
    fn validate(&self, _ctxt: &BookBuilder) -> BuildResult<()> {
        Ok(())
    }
}

struct RoomEntry {
    #[expect(dead_code)]
    name: String,
    conditions: BTreeMap<ConditionId, ConditionEntry>,
    nouns: BTreeMap<NounId, NounEntry>,
}

impl RoomEntry {
    fn validate(&self, ctxt: &BookBuilder) -> BuildResult<()> {
        validate_all(&self.conditions, |e| e.validate(ctxt))?;
        validate_all(&self.nouns, |e| e.validate(ctxt))?;
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
                    .map(|noun| (noun.id, NounEntry { desc: noun.desc })),
            )?,
        })
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

        builder.validate()?;

        Ok(builder)
    }
}

/// Validation helpers. Internal
impl BookBuilder {
    fn validate(&self) -> BuildResult<()> {
        validate_all(&self.cast, |e| e.validate(self))?;
        validate_all(&self.talkers, |e| e.validate(self))?;
        validate_all(&self.verbs, |e| e.validate(self))?;
        validate_all(&self.rooms, |e| e.validate(self))?;
        Ok(())
    }

    fn contains_cast(&self, cast_id: &CastId) -> bool {
        self.cast.contains_key(cast_id)
    }
}
