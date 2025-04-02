use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::text;
use crate::book::{self, Book};

fn get_only_item<I: IntoIterator>(items: I) -> I::Item {
    let mut iter = items.into_iter();
    let item = iter.next().expect("Expected exactly one item");
    assert!(iter.next().is_none(), "Expected exactly one item");
    item
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
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

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
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

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
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

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct RoomId(String);

impl From<book::RoomId> for RoomId {
    fn from(room_id: book::RoomId) -> Self {
        RoomId(format!("room-{}", room_id.room_num()))
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct RoleId(String);

impl From<book::RoleId> for RoleId {
    fn from(role_id: book::RoleId) -> Self {
        RoleId(role_id.as_str().to_string())
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
}

impl TextStyle {
    pub fn is_default(&self) -> bool {
        !self.bold && !self.italic
    }
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
pub struct RichText {
    pub items: Vec<TextPiece>,
}

impl From<text::RichText> for RichText {
    fn from(text: text::RichText) -> Self {
        let items = text.items().iter().map(Into::into).collect();
        RichText { items }
    }
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Conversation {
    pub lines: Vec<Line>,
}

impl From<book::Conversation<'_>> for Conversation {
    fn from(conv: book::Conversation<'_>) -> Self {
        Self {
            lines: conv.lines().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Noun {
    pub noun_id: u32,
    pub noun_name: Option<String>,
    pub conversations: Vec<ConvId>,
}

impl From<book::Noun<'_>> for Noun {
    fn from(noun: book::Noun<'_>) -> Self {
        Self {
            noun_id: noun.id().noun_num().into(),
            noun_name: noun.desc().map(ToOwned::to_owned),
            conversations: noun.conversations().map(|conv| conv.id().into()).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Room {
    pub room_id: u32,
    pub room_name: String,
    pub nouns: Vec<NounId>,
    pub cutscenes: Vec<ConvId>,
}

impl From<book::Room<'_>> for Room {
    fn from(room: book::Room<'_>) -> Self {
        Self {
            room_id: room.id().room_num().into(),
            room_name: room.name().to_string(),
            nouns: room
                .nouns()
                .filter(|noun| !noun.is_cutscene())
                .map(|noun| noun.id().into())
                .collect(),
            cutscenes: room
                .nouns()
                .filter(|noun| noun.is_cutscene())
                .map(|noun| get_only_item(noun.conversations()).id().into())
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Role {
    pub name: String,
}

impl From<book::Role<'_>> for Role {
    fn from(role: book::Role<'_>) -> Self {
        Self {
            name: role.name().to_string(),
        }
    }
}

// Script query types:
//
// - Find conversations by role
#[derive(Serialize, Deserialize, Debug)]
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
                .filter(|noun| !noun.is_cutscene())
                .map(|noun| (noun.id().into(), noun.into()))
                .collect(),
            conversations: book
                .conversations()
                .map(|conv| (conv.id().into(), conv.into()))
                .collect(),
        }
    }
}
