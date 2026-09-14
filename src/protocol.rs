//! Newline-delimited JSON messages shared by the client and server.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const GENERAL_ROOM: &str = "general";
pub const MIN_USERNAME_LENGTH: usize = 3;
pub const MAX_USERNAME_LENGTH: usize = 24;
pub const MAX_MESSAGE_LENGTH: usize = 1_000;
pub const MAX_HISTORY_LIMIT: u32 = 100;
pub const MAX_WIRE_LINE_LENGTH: usize = 8_192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Register { username: String, password: String },
    Chat { content: String },
    Help,
    ListUsers,
    DirectMessage { to: String, content: String },
    History { limit: Option<u32> },
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Authenticated {
        username: String,
    },
    Chat {
        room: String,
        username: String,
        content: String,
        timestamp: DateTime<Utc>,
    },
    Notice {
        message: String,
    },
    UserList {
        users: Vec<String>,
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

pub fn validate_username(username: &str) -> Result<(), String> {
    let length = username.chars().count();
    if !(MIN_USERNAME_LENGTH..=MAX_USERNAME_LENGTH).contains(&length) {
        return Err(format!(
            "Username must contain {MIN_USERNAME_LENGTH} to {MAX_USERNAME_LENGTH} characters"
        ));
    }
    if !username
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err("Username may contain only letters, numbers, _ and -".to_owned());
    }
    Ok(())
}

pub fn validate_message(content: &str) -> Result<(), String> {
    let length = content.chars().count();
    if length == 0 {
        return Err("Message cannot be empty".to_owned());
    }
    if length > MAX_MESSAGE_LENGTH {
        return Err(format!(
            "Message cannot exceed {MAX_MESSAGE_LENGTH} characters"
        ));
    }
    Ok(())
}

pub fn validate_history_limit(limit: Option<u32>) -> Result<u32, String> {
    let limit = limit.unwrap_or(20);
    if !(1..=MAX_HISTORY_LIMIT).contains(&limit) {
        return Err(format!(
            "History count must be between 1 and {MAX_HISTORY_LIMIT}"
        ));
    }
    Ok(limit)
}
