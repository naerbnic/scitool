use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::text;
use schemars::JsonSchema;
use scitool_book::{self as book, Book};

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, JsonSchema)]
#[serde(transparent)]
pub struct LineId(String);

impl From<book::LineId> for LineId {
    fn from(line_id: book::LineId) -> Self {
        LineId(format!(
            "line-{}-{}-{}-{}-{}",
            line_id.room_num(),
            line_id.noun_num(),
            line_id.verb_num(),
            line_id.condition_num(),
            line_id.sequence_num()
        ))
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, JsonSchema)]
#[serde(transparent)]
pub struct ConvId(String);

impl From<book::ConversationId> for ConvId {
    fn from(conv_id: book::ConversationId) -> Self {
        ConvId(format!(
            "conv-{}-{}-{}-{}",
            conv_id.room_num(),
            conv_id.noun_num(),
            conv_id.verb_num(),
            conv_id.condition_num()
        ))
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, JsonSchema)]
#[serde(transparent)]
pub struct NounId(String);

impl From<book::NounId> for NounId {
    fn from(noun_id: book::NounId) -> Self {
        NounId(format!(
            "noun-{}-{}",
            noun_id.room_num(),
            noun_id.noun_num()
        ))
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, JsonSchema)]
#[serde(transparent)]
pub struct RoomId(String);

impl From<book::RoomId> for RoomId {
    fn from(room_id: book::RoomId) -> Self {
        RoomId(format!("room-{}", room_id.room_num()))
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, JsonSchema)]
#[serde(transparent)]
pub struct RoleId(String);

impl From<book::RoleId> for RoleId {
    fn from(role_id: book::RoleId) -> Self {
        RoleId(role_id.as_str().to_string())
    }
}

#[derive(Serialize, Deserialize, Default, Debug, JsonSchema)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
}

impl TextStyle {
    pub fn is_default(&self) -> bool {
        !self.bold && !self.italic
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct TextPiece {
    pub text: String,
    #[serde(default, skip_serializing_if = "TextStyle::is_default")]
    pub style: TextStyle,
}

impl From<&text::TextItem> for TextPiece {
    fn from(item: &text::TextItem) -> Self {
        TextPiece {
            text: item.text().to_string(),
            style: TextStyle {
                bold: item.style().bold(),
                italic: item.style().italic(),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct RichText {
    pub items: Vec<TextPiece>,
}

impl From<text::RichText> for RichText {
    fn from(text: text::RichText) -> Self {
        let items = text.items().iter().map(Into::into).collect();
        RichText { items }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct Line {
    pub id: LineId,
    pub role: RoleId,
    pub text: RichText,
}

impl From<book::Line<'_>> for Line {
    fn from(line: book::Line<'_>) -> Self {
        Self {
            id: line.id().into(),
            role: line.role().id().into(),
            text: text::RichText::from_msg_text(line.text()).into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct Conversation {
    pub noun: NounId,
    pub verb: Option<String>,
    pub cond: Option<String>,
    pub lines: Vec<Line>,
}

impl From<book::Conversation<'_>> for Conversation {
    fn from(conv: book::Conversation<'_>) -> Self {
        Self {
            noun: conv.noun().id().into(),
            verb: conv.verb().map(|v| v.name().to_string()),
            cond: conv
                .condition()
                .and_then(|c| c.desc().map(|d| d.to_string())),
            lines: conv.lines().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct Noun {
    pub room_id: RoomId,
    pub noun_id: u32,
    pub noun_title: RichText,
    pub noun_name: Option<String>,
    pub conversations: Vec<ConvId>,
}

impl From<book::Noun<'_>> for Noun {
    fn from(noun: book::Noun<'_>) -> Self {
        Self {
            room_id: noun.room().id().into(),
            noun_id: noun.id().noun_num().into(),
            noun_title: text::make_noun_title(&noun).into(),
            noun_name: noun.desc().map(ToOwned::to_owned),
            conversations: noun.conversations().map(|conv| conv.id().into()).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct Room {
    pub room_id: u32,
    pub room_title: RichText,
    pub nouns: Vec<NounId>,
}

impl From<book::Room<'_>> for Room {
    fn from(room: book::Room<'_>) -> Self {
        Self {
            room_id: room.id().room_num().into(),
            room_title: text::make_room_title(&room).into(),
            nouns: room.nouns().map(|noun| noun.id().into()).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct Role {
    pub name: String,
    pub short_name: String,
}

impl From<book::Role<'_>> for Role {
    fn from(role: book::Role<'_>) -> Self {
        Self {
            name: role.name().to_string(),
            short_name: role.short_name().to_string(),
        }
    }
}

// Script query types:
//
// - Find conversations by role
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct GameScript {
    roles: BTreeMap<RoleId, Role>,
    rooms: BTreeMap<RoomId, Room>,
    nouns: BTreeMap<NounId, Noun>,
    conversations: BTreeMap<ConvId, Conversation>,
}

impl GameScript {
    pub fn from_book(book: &Book) -> Self {
        Self {
            roles: book
                .roles()
                .map(|role| (role.id().into(), role.into()))
                .collect(),
            rooms: book
                .rooms()
                .map(|room| (room.id().into(), room.into()))
                .collect(),
            nouns: book
                .nouns()
                .map(|noun| (noun.id().into(), noun.into()))
                .collect(),
            conversations: book
                .conversations()
                .map(|conv| (conv.id().into(), conv.into()))
                .collect(),
        }
    }

    pub fn json_schema() -> anyhow::Result<String> {
        Ok(serde_json::to_string(&schemars::schema_for!(Self))?)
    }
}
