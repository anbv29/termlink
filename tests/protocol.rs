use chrono::{TimeZone, Utc};
use termlink::protocol::{self, ClientMessage, ServerMessage};

#[test]
fn client_message_round_trips_through_json() {
    let original = ClientMessage::DirectMessage {
        to: "alice".to_owned(),
        content: "hello".to_owned(),
    };
    let json = protocol::encode(&original).unwrap();
    let decoded: ClientMessage = protocol::decode(&json).unwrap();

    assert_eq!(decoded, original);
    assert!(!json.contains('\n'));
}

#[test]
fn timestamped_server_message_round_trips_through_json() {
    let original = ServerMessage::Chat {
        room: "general".to_owned(),
        username: "alice".to_owned(),
        content: "hello".to_owned(),
        timestamp: Utc.with_ymd_and_hms(2026, 9, 14, 12, 30, 0).unwrap(),
    };
    let json = protocol::encode(&original).unwrap();
    let decoded: ServerMessage = protocol::decode(&json).unwrap();

    assert_eq!(decoded, original);
}
