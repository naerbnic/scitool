use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    pub room_name: Option<String>,
    pub nouns: Vec<Noun>,
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
