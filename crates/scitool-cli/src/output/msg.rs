//! Data types used for output from scitool.

use serde::{Deserialize, Serialize};

/// A message identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MessageId {
    pub room: u16,
    pub noun: u8,
    pub verb: u8,
    pub condition: u8,
    pub sequence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Message {
    pub id: MessageId,
    pub talker: u8,
    pub text: String,
}

/// The top level structure for a message output file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MessageFile {
    pub messages: Vec<Message>,
}
