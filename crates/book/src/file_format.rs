//! This module defines a file format for SCI books, to be reused and accesssed from
//! different libraries and languages.
//!
//! The file format is a JSON file that contains the book's structure. It does not
//! contain any indexes of the internal structure, but provides enough information
//! to reconstruct the book's structure and access its contents.

use std::collections::HashMap;
use std::hash::Hash;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    Book, ConditionId, ConversationId, LineId, NounId, RoleId, RoomId, VerbId,
    rich_text::{RichText, TextItem},
};

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
            role: *role_index_map
                .get(&line.role().id())
                .expect("Line's role should be in the role index map"),
            text: build_line_text(line.text()),
            parent_conversation: *conversation_index_map
                .get(&line.conversation().id())
                .expect("Line's conversation should be in the conversation index map"),
        },
    ).0
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
    let lines = build_lines(book, &role_index_map, &conversation_index_map);
    BookFormat {
        project_name: book.project_name().to_string(),
        roles,
        verbs,
        rooms,
        conditions,
        nouns,
        conversations,
        lines,
    }
}

pub fn serialize_book<S>(book: &Book, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    format_book(book).serialize(serializer)
}
