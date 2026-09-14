use termlink::commands::{Command, ParsedInput, parse_input};

#[test]
fn parses_direct_message_with_multiple_words() {
    assert_eq!(
        parse_input("/dm alice hello from Rust").unwrap(),
        ParsedInput::Command(Command::DirectMessage {
            username: "alice".to_owned(),
            message: "hello from Rust".to_owned(),
        })
    );
}

#[test]
fn parses_optional_history_count() {
    assert_eq!(
        parse_input("/history 15").unwrap(),
        ParsedInput::Command(Command::History { limit: Some(15) })
    );
    assert_eq!(
        parse_input("/history").unwrap(),
        ParsedInput::Command(Command::History { limit: None })
    );
}

#[test]
fn rejects_incomplete_and_unknown_commands() {
    assert!(parse_input("/dm alice").is_err());
    assert!(parse_input("/dance").is_err());
}

#[test]
fn plain_text_remains_a_public_chat_message() {
    assert_eq!(
        parse_input("  hello general room  ").unwrap(),
        ParsedInput::Chat("hello general room".to_owned())
    );
}
