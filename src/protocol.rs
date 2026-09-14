//! Newline-delimited JSON messages shared by the client and server.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const GENERAL_ROOM: &str = "general";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Join { username: String },
    Chat { content: String },
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Chat {
        room: String,
        username: String,
        content: String,
        timestamp: DateTime<Utc>,
    },
    Notice {
        message: String,
    },
    Error {
        message: String,
    },
}

pub fn encode<T: Serialize>(message: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(message)
}

pub fn decode<'a, T: Deserialize<'a>>(line: &'a str) -> Result<T, serde_json::Error> {
    serde_json::from_str(line)
}
