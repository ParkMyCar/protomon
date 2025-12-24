//! Integration tests for unknown field preservation.

extern crate alloc;

use bytes::Bytes;
use protomon::codec::{ProtoMessage, ProtoString};
use protomon::ProtoMessage;

/// Message that preserves unknown fields
#[derive(Debug, Clone, Default, ProtoMessage)]
pub struct MessageWithUnknown {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(tag = 2)]
    pub age: i32,
    /// Unknown fields for round-trip compatibility
    #[proto(unknown)]
    pub _unknown: Bytes,
}

/// Message without unknown field preservation
#[derive(Debug, Clone, Default, ProtoMessage)]
pub struct MessageWithoutUnknown {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(tag = 2)]
    pub age: i32,
}

/// Extended version with additional fields (simulating a newer schema)
#[derive(Debug, Clone, Default, ProtoMessage)]
pub struct ExtendedMessage {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(tag = 2)]
    pub age: i32,
    #[proto(tag = 3, optional)]
    pub email: Option<ProtoString>,
    #[proto(tag = 4)]
    pub score: i64,
}

#[test]
fn test_unknown_fields_preserved() {
    // Create an extended message with extra fields
    let mut extended = ExtendedMessage::default();
    extended.name = ProtoString::from("Alice");
    extended.age = 30;
    extended.email = Some(ProtoString::from("alice@example.com"));
    extended.score = 100;

    // Encode the extended message
    let mut buf = Vec::new();
    extended.encode_message(&mut buf);
    let encoded_bytes = Bytes::from(buf.clone());

    // Decode into a message that preserves unknown fields
    let msg_with_unknown =
        MessageWithUnknown::decode_message(encoded_bytes.clone()).expect("decode failed");

    // The known fields should be decoded correctly
    assert_eq!(msg_with_unknown.name.as_str(), "Alice");
    assert_eq!(msg_with_unknown.age, 30);

    // The _unknown field should contain the extra fields (email and score)
    assert!(!msg_with_unknown._unknown.is_empty());

    // Re-encode the message with unknown fields
    let mut buf2 = Vec::new();
    msg_with_unknown.encode_message(&mut buf2);

    // The re-encoded message should be identical to the original
    assert_eq!(buf, buf2);

    // Decode back into the extended message to verify round-trip
    let decoded_extended =
        ExtendedMessage::decode_message(Bytes::from(buf2)).expect("decode failed");
    assert_eq!(decoded_extended.name.as_str(), "Alice");
    assert_eq!(decoded_extended.age, 30);
    assert_eq!(
        decoded_extended.email.as_ref().map(|s| s.as_str()),
        Some("alice@example.com")
    );
    assert_eq!(decoded_extended.score, 100);
}

#[test]
fn test_unknown_fields_not_preserved() {
    // Create an extended message
    let mut extended = ExtendedMessage::default();
    extended.name = ProtoString::from("Bob");
    extended.age = 25;
    extended.email = Some(ProtoString::from("bob@example.com"));
    extended.score = 200;

    // Encode the extended message
    let mut buf = Vec::new();
    extended.encode_message(&mut buf);
    let encoded_bytes = Bytes::from(buf.clone());
    let original_len = buf.len();

    // Decode into a message that does NOT preserve unknown fields
    let msg_without_unknown =
        MessageWithoutUnknown::decode_message(encoded_bytes).expect("decode failed");

    // The known fields should be decoded correctly
    assert_eq!(msg_without_unknown.name.as_str(), "Bob");
    assert_eq!(msg_without_unknown.age, 25);

    // Re-encode the message
    let mut buf2 = Vec::new();
    msg_without_unknown.encode_message(&mut buf2);

    // The re-encoded message should be SMALLER (missing the unknown fields)
    assert!(buf2.len() < original_len);

    // The unknown fields are lost
    let decoded =
        MessageWithoutUnknown::decode_message(Bytes::from(buf2)).expect("decode failed");
    assert_eq!(decoded.name.as_str(), "Bob");
    assert_eq!(decoded.age, 25);
}

#[test]
fn test_empty_unknown_fields() {
    // Create a message with only known fields
    let mut msg = MessageWithUnknown::default();
    msg.name = ProtoString::from("Charlie");
    msg.age = 35;

    // Encode and decode
    let mut buf = Vec::new();
    msg.encode_message(&mut buf);
    let decoded = MessageWithUnknown::decode_message(Bytes::from(buf)).expect("decode failed");

    // The _unknown field should be empty
    assert!(decoded._unknown.is_empty());
    assert_eq!(decoded.name.as_str(), "Charlie");
    assert_eq!(decoded.age, 35);
}

#[test]
fn test_unknown_fields_length_calculation() {
    // Create an extended message
    let mut extended = ExtendedMessage::default();
    extended.name = ProtoString::from("Dave");
    extended.age = 40;
    extended.email = Some(ProtoString::from("dave@example.com"));
    extended.score = 300;

    // Encode the extended message
    let mut buf = Vec::new();
    extended.encode_message(&mut buf);
    let encoded_bytes = Bytes::from(buf.clone());

    // Decode into a message that preserves unknown fields
    let msg_with_unknown =
        MessageWithUnknown::decode_message(encoded_bytes).expect("decode failed");

    // The calculated length should match the actual encoded length
    let calculated_len = msg_with_unknown.encoded_message_len();
    let mut test_buf = Vec::new();
    msg_with_unknown.encode_message(&mut test_buf);
    assert_eq!(calculated_len, test_buf.len());
}
