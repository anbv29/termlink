//! Shared code for the TermLink server and client.

pub mod auth;
pub mod commands;
pub mod database;
pub mod protocol;

/// The human-readable application name.
pub const APP_NAME: &str = "TermLink";
