//! This module defines a file format for SCI books, to be reused and accesssed from
//! different libraries and languages.
//!
//! The file format is a JSON file that contains the book's structure. It does not
//! contain any indexes of the internal structure, but provides enough information
//! to reconstruct the book's structure and access its contents.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Book;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct RoleIndex(usize);
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct TalkerIndex(usize);
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct VerbIndex(usize);
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct RoomIndex(usize);
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct ConditionIndex(usize);
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct NounIndex(usize);
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct ConversationIndex(usize);
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
struct LineIndex(usize);

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct BookFormat {
    #[serde(rename = "projectName")]
    project_name: String,
    roles: Vec<RoleItem>,
    talkers: Vec<TalkerItem>,
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
struct TalkerItem {
    id: String,
    num: u8,
    role: RoleIndex,
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
    talker: TalkerIndex,
    text: LineText,
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
    underline: bool,
}

