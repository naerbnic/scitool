//! This module defines a file format for SCI books, to be reused and accesssed from
//! different libraries and languages.
//!
//! The file format is a JSON file that contains the book's structure. It does not
//! contain any indexes of the internal structure, but provides enough information
//! to reconstruct the book's structure and access its contents.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use schemars::JsonSchema;
use scidev::common::{
    ConversationKey, RawConditionId, RawNounId, RawRoomId, RawSequenceId, RawVerbId,
};
use serde::{Deserialize, Serialize};

use crate::book::ids::RawTalkerId;
use crate::book::rich_text::TextStyle;
use crate::book::{
    Book, ConditionId, ConversationId, NounId, RoleEntry, RoleId, RoomEntry, RoomId, VerbEntry,
    VerbId,
    ids::RawRoleId,
    rich_text::{RichText, TextItem},
};
use crate::book::{ConditionEntry, ConversationEntry, LineEntry, NounEntry};

#[derive(Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct RoleIndex(usize);
#[derive(Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct VerbIndex(usize);
#[derive(Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct RoomIndex(usize);
#[derive(Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct ConditionIndex(usize);
#[derive(Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct NounIndex(usize);
#[derive(Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct ConversationIndex(usize);
#[derive(Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct LineIndex(usize);

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct BookFormat {
    #[serde(rename = "projectName")]
    project_name: String,
    roles: Vec<RoleItem>,
    verbs: Vec<VerbItem>,
    rooms: Vec<RoomItem>,
    conditions: Vec<ConditionItem>,
    nouns: Vec<NounItem>,
    conversations: Vec<ConversationItem>,
    lines: Vec<LineItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RoleItem {
    id: String,
    raw: String,
    name: String,
    #[serde(rename = "shortName")]
    short_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct VerbItem {
    id: String,
    num: u8,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RoomItem {
    id: String,
    num: u16,
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ConditionItem {
    id: String,
    num: u8,
    description: Option<String>,
    #[serde(rename = "parentRoom")]
    parent_room: RoomIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct NounItem {
    id: String,
    num: u8,
    description: Option<String>,
    is_cutscene: bool,
    #[serde(rename = "parentRoom")]
    parent_room: RoomIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ConversationItem {
    id: String,
    verb: Option<VerbIndex>,
    condition: Option<ConditionIndex>,
    #[serde(rename = "parentNoun")]
    parent_noun: NounIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct LineItem {
    id: String,
    num: u8,
    talker_num: u8,
    role: RoleIndex,
    text: LineText,
    #[serde(rename = "parentConversation")]
    parent_conversation: ConversationIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum LineText {
    Simple(String),
    Rich(Vec<LineSegment>),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum LineSegment {
    Plain(String),
    Rich(RichSegment),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RichSegment {
    text: String,
    style: RichStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RichStyle {
    bold: bool,
    italic: bool,
}

fn make_indexed_vec<I, IdentT, IdxT, ItemT, IdentF, IdxF, ItemF>(
    items: I,
    ident_fn: IdentF,
    idx_fn: IdxF,
    item_fn: ItemF,
) -> (Vec<ItemT>, HashMap<IdentT, IdxT>)
where
    I: IntoIterator,
    I::Item: Clone,
    IdentT: Hash + Eq + std::fmt::Debug,
    IdentF: Fn(I::Item) -> IdentT,
    IdxF: Fn(usize) -> IdxT,
    ItemF: Fn(I::Item) -> ItemT,
{
    let mut index_map = HashMap::new();
    let item_vec = items
        .into_iter()
        .enumerate()
        .inspect(|(index, entry)| {
            let old_value = index_map.insert(ident_fn(entry.clone()), idx_fn(*index));
            assert!(
                old_value.is_none(),
                "Duplicate identifier found: {:?}",
                ident_fn(entry.clone())
            );
        })
        .map(|(_, entry)| item_fn(entry))
        .collect();
    (item_vec, index_map)
}

fn build_roles(book: &Book) -> (Vec<RoleItem>, HashMap<RoleId, RoleIndex>) {
    make_indexed_vec(
        book.roles(),
        |role| role.id(),
        RoleIndex,
        |role| RoleItem {
            id: format!("role-{}", role.id().as_str()),
            raw: role.id().to_string(),
            name: role.name().to_string(),
            short_name: role.short_name().to_string(),
        },
    )
}

fn build_rooms(book: &Book) -> (Vec<RoomItem>, HashMap<RoomId, RoomIndex>) {
    make_indexed_vec(
        book.rooms(),
        |room| room.id(),
        RoomIndex,
        |room| RoomItem {
            id: room.id().to_string(),
            num: room.id().room_num(),
            name: room.name().map(String::from),
        },
    )
}

fn build_verbs(book: &Book) -> (Vec<VerbItem>, HashMap<VerbId, VerbIndex>) {
    make_indexed_vec(
        book.verbs(),
        |verb| verb.id(),
        VerbIndex,
        |verb| VerbItem {
            id: format!("verb-{}", verb.id().verb_num()),
            num: verb.id().verb_num(),
            name: verb.name().to_string(),
        },
    )
}
fn build_conditions(
    book: &Book,
    room_index_map: &HashMap<RoomId, RoomIndex>,
) -> (Vec<ConditionItem>, HashMap<ConditionId, ConditionIndex>) {
    make_indexed_vec(
        book.conditions(),
        |cond| cond.id(),
        ConditionIndex,
        |cond| ConditionItem {
            id: format!(
                "condition-{}-{}",
                cond.id().room_id().room_num(),
                cond.id().condition_num()
            ),
            num: cond.id().condition_num(),
            description: cond.desc().map(String::from),
            parent_room: *room_index_map
                .get(&cond.id().room_id())
                .expect("Condition's room should be in the room index map"),
        },
    )
}

fn build_nouns(
    book: &Book,
    room_index_map: &HashMap<RoomId, RoomIndex>,
) -> (Vec<NounItem>, HashMap<NounId, NounIndex>) {
    make_indexed_vec(
        book.nouns(),
        |noun| noun.id(),
        NounIndex,
        |noun| NounItem {
            id: noun.id().to_string(),
            num: noun.id().noun_num(),
            description: noun.desc().map(String::from),
            is_cutscene: noun.is_cutscene(),
            parent_room: *room_index_map
                .get(&noun.id().room_id())
                .expect("Noun's room should be in the room index map"),
        },
    )
}

fn build_conversations(
    book: &Book,
    verb_index_map: &HashMap<VerbId, VerbIndex>,
    condition_index_map: &HashMap<ConditionId, ConditionIndex>,
    noun_index_map: &HashMap<NounId, NounIndex>,
) -> (
    Vec<ConversationItem>,
    HashMap<ConversationId, ConversationIndex>,
) {
    make_indexed_vec(
        book.conversations(),
        |conv| conv.id(),
        ConversationIndex,
        |conv| ConversationItem {
            id: conv.id().to_string(),
            verb: conv.verb().map(|verb| {
                *verb_index_map
                    .get(&verb.id())
                    .expect("Conversation's verb should be in the verb index map")
            }),
            condition: conv.condition().map(|condition| {
                *condition_index_map
                    .get(&condition.id())
                    .expect("Conversation's condition should be in the condition index map")
            }),
            parent_noun: *noun_index_map
                .get(&conv.noun().id())
                .expect("Conversation's noun should be in the noun index map"),
        },
    )
}

fn build_lines(
    book: &Book,
    role_index_map: &HashMap<RoleId, RoleIndex>,
    conversation_index_map: &HashMap<ConversationId, ConversationIndex>,
) -> Vec<LineItem> {
    make_indexed_vec(
        book.lines(),
        |line| line.id(),
        LineIndex,
        |line| LineItem {
            id: line.id().to_string(),
            num: line.id().sequence_num(),
            talker_num: line.talker_num(),
            role: *role_index_map
                .get(&line.role().id())
                .expect("Line's role should be in the role index map"),
            text: build_line_text(line.text()),
            parent_conversation: *conversation_index_map
                .get(&line.conversation().id())
                .expect("Line's conversation should be in the conversation index map"),
        },
    )
    .0
}

fn build_line_text_segment(segment: &TextItem) -> LineSegment {
    if segment.style().is_plain() {
        LineSegment::Plain(segment.text().to_string())
    } else {
        LineSegment::Rich(RichSegment {
            text: segment.text().to_string(),
            style: RichStyle {
                bold: segment.style().bold(),
                italic: segment.style().italic(),
            },
        })
    }
}

fn build_line_text(text: &RichText) -> LineText {
    match text.items() {
        [] => LineText::Simple(String::new()),
        [item] if item.style().is_plain() => LineText::Simple(item.text().to_string()),
        _ => LineText::Rich(text.items().iter().map(build_line_text_segment).collect()),
    }
}

fn format_book(book: &Book) -> BookFormat {
    let (roles, role_index_map) = build_roles(book);
    let (verbs, verb_index_map) = build_verbs(book);
    let (rooms, room_index_map) = build_rooms(book);
    let (conditions, condition_index_map) = build_conditions(book, &room_index_map);
    let (nouns, noun_index_map) = build_nouns(book, &room_index_map);
    let (conversations, conversation_index_map) =
        build_conversations(book, &verb_index_map, &condition_index_map, &noun_index_map);
    BookFormat {
        project_name: book.project_name().to_string(),
        roles,
        verbs,
        rooms,
        conditions,
        nouns,
        conversations,
        lines: build_lines(book, &role_index_map, &conversation_index_map),
    }
}

pub fn serialize_book<S>(book: &Book, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    format_book(book).serialize(serializer)
}

fn make_roles(formatted_book: &BookFormat) -> BTreeMap<RawRoleId, RoleEntry> {
    let mut role_map: BTreeMap<RawRoleId, RoleEntry> = BTreeMap::new();
    for role in &formatted_book.roles {
        let role_id = RawRoleId::new(role.raw.clone());
        let entry = RoleEntry {
            name: role.name.clone(),
            short_name: role.short_name.clone(),
        };
        role_map.insert(role_id, entry);
    }
    role_map
}

fn make_verbs(formatted_book: &BookFormat) -> BTreeMap<RawVerbId, VerbEntry> {
    let verb_map: BTreeMap<RawVerbId, VerbEntry> = formatted_book
        .verbs
        .iter()
        .map(|verb| {
            (
                RawVerbId::new(verb.num),
                VerbEntry {
                    name: verb.name.clone(),
                },
            )
        })
        .collect();
    verb_map
}

fn make_rooms(
    formatted_book: &BookFormat,
    mut noun_map: BTreeMap<RoomId, BTreeMap<RawNounId, NounEntry>>,
    mut condition_map: BTreeMap<RoomId, BTreeMap<RawConditionId, ConditionEntry>>,
) -> BTreeMap<RawRoomId, RoomEntry> {
    let mut rooms: BTreeMap<RawRoomId, RoomEntry> = BTreeMap::new();
    for room in &formatted_book.rooms {
        let raw_room_id = RawRoomId::new(room.num);
        let room_id = RoomId::from_raw(raw_room_id);
        let entry = RoomEntry {
            name: room.name.clone(),
            nouns: noun_map.remove(&room_id).unwrap_or_default(),
            conditions: condition_map.remove(&room_id).unwrap_or_default(),
        };
        rooms.insert(raw_room_id, entry);
    }
    rooms
}

fn make_condition_map(
    formatted_book: &BookFormat,
    room_ids: &[RoomId],
) -> BTreeMap<RoomId, BTreeMap<RawConditionId, ConditionEntry>> {
    let mut condition_map: BTreeMap<RoomId, BTreeMap<RawConditionId, ConditionEntry>> =
        BTreeMap::new();
    for cond in &formatted_book.conditions {
        let room_id = room_ids[cond.parent_room.0];
        let raw_cond_id = RawConditionId::new(cond.num);
        let entry = ConditionEntry {
            desc: cond.description.clone(),
        };
        condition_map
            .entry(room_id)
            .or_default()
            .insert(raw_cond_id, entry);
    }
    condition_map
}

fn make_noun_map(
    formatted_book: &BookFormat,
    room_ids: &[RoomId],
    mut conv_map: BTreeMap<NounId, BTreeMap<ConversationKey, ConversationEntry>>,
) -> BTreeMap<RoomId, BTreeMap<RawNounId, NounEntry>> {
    let mut noun_map: BTreeMap<RoomId, BTreeMap<RawNounId, NounEntry>> = BTreeMap::new();
    for noun in &formatted_book.nouns {
        let room_id = room_ids[noun.parent_room.0];
        let raw_noun_id = RawNounId::new(noun.num);
        let noun_id = NounId::from_room_raw(room_id, raw_noun_id);
        let entry = NounEntry {
            desc: noun.description.clone(),
            is_cutscene: noun.is_cutscene,
            conversations: conv_map.remove(&noun_id).unwrap_or_default(),
        };
        noun_map
            .entry(room_id)
            .or_default()
            .insert(raw_noun_id, entry);
    }
    noun_map
}

fn make_conv_map(
    formatted_book: &BookFormat,
    verb_ids: &[VerbId],
    condition_ids: &[ConditionId],
    noun_ids: &[NounId],
    mut line_map: BTreeMap<ConversationId, BTreeMap<RawSequenceId, LineEntry>>,
) -> BTreeMap<NounId, BTreeMap<ConversationKey, ConversationEntry>> {
    let mut conv_map: BTreeMap<NounId, BTreeMap<ConversationKey, ConversationEntry>> =
        BTreeMap::new();
    for conv in &formatted_book.conversations {
        let noun_id = noun_ids[conv.parent_noun.0];
        let verb_id = conv
            .verb
            .map_or(RawVerbId::new(0), |v| verb_ids[v.0].raw_id());
        let condition_id = conv
            .condition
            .map_or(RawConditionId::new(0), |c| condition_ids[c.0].raw_id());
        let key = ConversationKey::new(verb_id, condition_id);
        let entry = ConversationEntry {
            lines: line_map
                .remove(&ConversationId::from_noun_key(noun_id, key))
                .unwrap_or_default(),
        };
        conv_map.entry(noun_id).or_default().insert(key, entry);
    }
    conv_map
}

fn make_line_map(
    formatted_book: &BookFormat,
    role_ids: &[RoleId],
    conversation_ids: &[ConversationId],
) -> BTreeMap<ConversationId, BTreeMap<RawSequenceId, LineEntry>> {
    let mut line_map: BTreeMap<ConversationId, BTreeMap<RawSequenceId, LineEntry>> =
        BTreeMap::new();
    for line in &formatted_book.lines {
        let conversation_id = conversation_ids[line.parent_conversation.0];
        let role = role_ids[line.role.0].raw_id().clone();
        let sequence_id = RawSequenceId::new(line.num);
        let entry = LineEntry {
            role,
            talker: RawTalkerId::new(line.talker_num),
            text: deserialize_rich_text(&line.text),
        };
        line_map
            .entry(conversation_id)
            .or_default()
            .insert(sequence_id, entry);
    }
    line_map
}

fn deserialize_rich_text(rich_text_format: &LineText) -> RichText {
    let mut builder = RichText::builder();
    match rich_text_format {
        LineText::Simple(text) => {
            builder.add_plain_text(text);
        }
        LineText::Rich(segments) => {
            for segment in segments {
                match segment {
                    LineSegment::Plain(text) => {
                        builder.add_plain_text(text);
                    }
                    LineSegment::Rich(rich) => {
                        builder.add_text(
                            &rich.text,
                            TextStyle::of_plain()
                                .set_bold(rich.style.bold)
                                .set_italic(rich.style.italic),
                        );
                    }
                }
            }
        }
    }
    builder.build()
}

pub fn deserialize_book<'de, D>(deserializer: D) -> Result<Book, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let formatted_book = BookFormat::deserialize(deserializer)?;

    // Create Index to ID maps (in the form of vectors)
    let role_ids: Vec<RoleId> = formatted_book
        .roles
        .iter()
        .map(|role| RoleId::from_raw(RawRoleId::new(role.id.clone())))
        .collect();
    let verb_ids: Vec<VerbId> = formatted_book
        .verbs
        .iter()
        .map(|verb| VerbId::from_raw(RawVerbId::new(verb.num)))
        .collect();
    let room_ids: Vec<RoomId> = formatted_book
        .rooms
        .iter()
        .map(|room| RoomId::from_raw(RawRoomId::new(room.num)))
        .collect();
    let condition_ids: Vec<ConditionId> = formatted_book
        .conditions
        .iter()
        .map(|cond| {
            ConditionId::from_room_raw(room_ids[cond.parent_room.0], RawConditionId::new(cond.num))
        })
        .collect();
    let noun_ids: Vec<NounId> = formatted_book
        .nouns
        .iter()
        .map(|noun| NounId::from_room_raw(room_ids[noun.parent_room.0], RawNounId::new(noun.num)))
        .collect();
    let conversation_ids: Vec<ConversationId> = formatted_book
        .conversations
        .iter()
        .map(|conv| {
            ConversationId::from_noun_key(
                noun_ids[conv.parent_noun.0],
                ConversationKey::new(
                    conv.verb
                        .map_or(RawVerbId::new(0), |v| verb_ids[v.0].raw_id()),
                    conv.condition
                        .map_or(RawConditionId::new(0), |c| condition_ids[c.0].raw_id()),
                ),
            )
        })
        .collect();

    let rooms = make_rooms(
        &formatted_book,
        make_noun_map(
            &formatted_book,
            &room_ids,
            make_conv_map(
                &formatted_book,
                &verb_ids,
                &condition_ids,
                &noun_ids,
                make_line_map(&formatted_book, &role_ids, &conversation_ids),
            ),
        ),
        make_condition_map(&formatted_book, &room_ids),
    );

    let verbs = make_verbs(&formatted_book);

    let roles = make_roles(&formatted_book);

    Ok(Book {
        project_name: formatted_book.project_name,
        roles,
        verbs,
        rooms,
    })
}

#[must_use]
pub fn json_schema(pretty_print: bool) -> String {
    let schema = schemars::schema_for!(BookFormat);
    let schema = schema.as_value();

    if pretty_print {
        serde_json::to_string_pretty(&schema)
    } else {
        serde_json::to_string(&schema)
    }
    .expect("Failed to serialize schema to JSON")
}
