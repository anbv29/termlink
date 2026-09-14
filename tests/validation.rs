use termlink::auth::validate_password;
use termlink::protocol::{validate_history_limit, validate_message, validate_username};

#[test]
fn accepts_safe_username_and_rejects_invalid_ones() {
    assert!(validate_username("rust_user-7").is_ok());
    assert!(validate_username("ab").is_err());
    assert!(validate_username("name with spaces").is_err());
}

#[test]
fn enforces_message_length() {
    assert!(validate_message("hello").is_ok());
    assert!(validate_message("").is_err());
    assert!(validate_message(&"x".repeat(1_001)).is_err());
}

#[test]
fn bounds_history_requests() {
    assert_eq!(validate_history_limit(None).unwrap(), 20);
    assert!(validate_history_limit(Some(0)).is_err());
    assert!(validate_history_limit(Some(101)).is_err());
}

#[test]
fn enforces_password_length_without_storing_passwords() {
    assert!(validate_password("eight888").is_ok());
    assert!(validate_password("short").is_err());
    assert!(validate_password(&"x".repeat(129)).is_err());
}
