use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::text;
use crate::book::{self, Book};

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct LineId(String);

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct ConvId(String);

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct NounId(String);

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct RoomId(String);

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct RoleId(String);

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

#[derive(Serialize, Deserialize, Debug)]
pub struct RichText {
    pub items: Vec<TextPiece>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Line {
    pub id: LineId,
    pub role: RoleId,
    pub text: RichText,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Conversation {
    pub lines: Vec<Line>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Noun {
    pub noun_id: u32,
    pub noun_name: Option<String>,
    pub conversations: Vec<ConvId>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Room {
    pub room_id: u32,
    pub room_name: String,
    pub nouns: Vec<NounId>,
    pub cutscenes: Vec<ConvId>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Role {
    pub name: String,
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

fn serialize_text(text: &text::RichText) -> RichText {
    let items = text
        .items()
        .iter()
        .map(|item| TextPiece {
            text: item.text().to_string(),
            style: TextStyle {
                bold: item.style().bold(),
                italic: item.style().italic(),
            },
        })
        .collect();
    RichText { items }
}

fn convert_book_role_id(book_role_id: book::RoleId) -> RoleId {
    RoleId(book_role_id.as_str().to_string())
}
fn convert_book_room_id(book_room_id: book::RoomId) -> RoomId {
    RoomId(format!("room-{}", book_room_id.room_num()))
}
fn convert_book_noun_id(book_noun_id: book::NounId) -> NounId {
    NounId(format!(
        "noun-{}-{}",
        book_noun_id.room_num(),
        book_noun_id.noun_num()
    ))
}
fn convert_book_conv_id(book_conv_id: book::ConversationId) -> ConvId {
    ConvId(format!(
        "conv-{}-{}-{}-{}",
        book_conv_id.room_num(),
        book_conv_id.noun_num(),
        book_conv_id.verb_num(),
        book_conv_id.condition_num()
    ))
}
fn convert_book_line_id(book_line_id: book::LineId) -> LineId {
    LineId(format!(
        "line-{}-{}-{}-{}-{}",
        book_line_id.room_num(),
        book_line_id.noun_num(),
        book_line_id.verb_num(),
        book_line_id.condition_num(),
        book_line_id.sequence_num()
    ))
}

impl GameScript {
    pub fn from_book(book: &Book) -> Self {
        let roles = book
            .roles()
            .map(|role| {
                let role_id = RoleId(role.id().as_str().to_string());
                let role = Role {
                    name: role.name().to_string(),
                };
                (role_id, role)
            })
            .collect();
        let rooms = book
            .rooms()
            .map(|room| {
                let room_id = convert_book_room_id(room.id());
                let room = Room {
                    room_id: room.id().room_num().into(),
                    room_name: room.name().to_string(),
                    nouns: room
                        .nouns()
                        .filter(|noun| !noun.is_cutscene())
                        .map(|noun| convert_book_noun_id(noun.id()))
                        .collect(),
                    cutscenes: room
                        .nouns()
                        .filter(|noun| noun.is_cutscene())
                        .map(|noun| {
                            assert_eq!(noun.conversations().count(), 1);
                            let conv = noun.conversations().next().unwrap();
                            convert_book_conv_id(conv.id())
                        })
                        .collect(),
                };
                (room_id, room)
            })
            .collect();
        let nouns = book
            .nouns()
            .filter(|noun| !noun.is_cutscene())
            .map(|noun| {
                let noun_id = convert_book_noun_id(noun.id());
                let noun = Noun {
                    noun_id: noun.id().noun_num().into(),
                    noun_name: noun.desc().map(ToOwned::to_owned),
                    conversations: noun
                        .conversations()
                        .map(|conv| convert_book_conv_id(conv.id()))
                        .collect(),
                };
                (noun_id, noun)
            })
            .collect();
        let conversations = book
            .conversations()
            .map(|conv| {
                let conv_id = convert_book_conv_id(conv.id());
                let lines = conv
                    .lines()
                    .map(|line| {
                        let line_id = convert_book_line_id(line.id());
                        let role_id = convert_book_role_id(line.role().id());
                        let text = serialize_text(&text::RichText::from_msg_text(line.text()));
                        Line {
                            id: line_id,
                            role: role_id,
                            text,
                        }
                    })
                    .collect();
                (conv_id, Conversation { lines })
            })
            .collect();
        Self {
            roles,
            rooms,
            nouns,
            conversations,
        }
    }
}
