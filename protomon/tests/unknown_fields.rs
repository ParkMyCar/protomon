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
    let extended = ExtendedMessage {
        name: ProtoString::from("Alice"),
        age: 30,
        email: Some(ProtoString::from("alice@example.com")),
        score: 100,
    };

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
    let extended = ExtendedMessage {
        name: ProtoString::from("Bob"),
        age: 25,
        email: Some(ProtoString::from("bob@example.com")),
        score: 200,
    };

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
    let decoded = MessageWithoutUnknown::decode_message(Bytes::from(buf2)).expect("decode failed");
    assert_eq!(decoded.name.as_str(), "Bob");
    assert_eq!(decoded.age, 25);
}

#[test]
fn test_empty_unknown_fields() {
    // Create a message with only known fields
    let msg = MessageWithUnknown {
        name: ProtoString::from("Charlie"),
        age: 35,
        ..Default::default()
    };

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
    let extended = ExtendedMessage {
        name: ProtoString::from("Dave"),
        age: 40,
        email: Some(ProtoString::from("dave@example.com")),
        score: 300,
    };

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

/// Message with various wire types for testing unknown field preservation
#[derive(Debug, Clone, Default, ProtoMessage)]
pub struct MultiWireTypeMessage {
    #[proto(tag = 1)]
    pub varint_field: i32,
    #[proto(tag = 2)]
    pub fixed64_field: protomon::codec::Fixed64,
    #[proto(tag = 3)]
    pub string_field: ProtoString,
    #[proto(tag = 4)]
    pub fixed32_field: protomon::codec::Fixed32,
    #[proto(tag = 5)]
    pub sint32_field: protomon::codec::Sint32,
    #[proto(tag = 6)]
    pub bytes_field: protomon::codec::ProtoBytes,
}

/// Minimal message that preserves unknown fields
#[derive(Debug, Clone, Default, ProtoMessage)]
pub struct MinimalWithUnknown {
    #[proto(tag = 1)]
    pub varint_field: i32,
    #[proto(unknown)]
    pub _unknown: Bytes,
}

#[test]
fn test_unknown_fields_multiple_wire_types() {
    // Create a message with all wire types
    let multi = MultiWireTypeMessage {
        varint_field: 42,
        fixed64_field: protomon::codec::Fixed64(0x123456789ABCDEF0),
        string_field: ProtoString::from("hello"),
        fixed32_field: protomon::codec::Fixed32(0x12345678),
        sint32_field: protomon::codec::Sint32(-100),
        bytes_field: protomon::codec::ProtoBytes::from(&[1u8, 2, 3, 4, 5][..]),
    };

    // Encode the full message
    let mut buf = Vec::new();
    multi.encode_message(&mut buf);
    let original_bytes = Bytes::from(buf.clone());

    // Decode into minimal message (only tag 1 is known)
    let minimal =
        MinimalWithUnknown::decode_message(original_bytes.clone()).expect("decode failed");

    // Known field should be decoded
    assert_eq!(minimal.varint_field, 42);

    // Unknown fields should be preserved
    assert!(!minimal._unknown.is_empty());

    // Re-encode and verify round-trip
    let mut buf2 = Vec::new();
    minimal.encode_message(&mut buf2);

    // Decode back to full message
    let decoded_multi =
        MultiWireTypeMessage::decode_message(Bytes::from(buf2)).expect("decode failed");

    assert_eq!(decoded_multi.varint_field, 42);
    assert_eq!(decoded_multi.fixed64_field.0, 0x123456789ABCDEF0);
    assert_eq!(decoded_multi.string_field.as_str(), "hello");
    assert_eq!(decoded_multi.fixed32_field.0, 0x12345678);
    assert_eq!(decoded_multi.sint32_field.0, -100);
    assert_eq!(decoded_multi.bytes_field.as_ref(), &[1, 2, 3, 4, 5]);
}

#[test]
fn test_unknown_fields_multiple_unknown_fields() {
    // Use MultiWireTypeMessage to test multiple unknown fields
    // MinimalWithUnknown knows tag 1 as varint, which matches MultiWireTypeMessage's tag 1
    let multi = MultiWireTypeMessage {
        varint_field: 123,                            // tag 1 - known
        fixed64_field: protomon::codec::Fixed64(999), // tag 2 - unknown
        string_field: ProtoString::from("test"),      // tag 3 - unknown
        fixed32_field: protomon::codec::Fixed32(456), // tag 4 - unknown
        sint32_field: protomon::codec::Sint32(-50),   // tag 5 - unknown
        bytes_field: protomon::codec::ProtoBytes::from(&[10u8, 20, 30][..]), // tag 6 - unknown
    };

    // Encode
    let mut buf = Vec::new();
    multi.encode_message(&mut buf);

    // Decode into message that only knows tag 1 (varint)
    let minimal =
        MinimalWithUnknown::decode_message(Bytes::from(buf.clone())).expect("decode failed");

    // Tag 1 should be decoded correctly
    assert_eq!(minimal.varint_field, 123);

    // We should have preserved multiple unknown fields (tags 2, 3, 4, 5, 6)
    assert!(!minimal._unknown.is_empty());

    // Re-encode
    let mut buf2 = Vec::new();
    minimal.encode_message(&mut buf2);

    // Verify all fields are recovered
    let recovered = MultiWireTypeMessage::decode_message(Bytes::from(buf2)).expect("decode failed");
    assert_eq!(recovered.varint_field, 123);
    assert_eq!(recovered.fixed64_field.0, 999);
    assert_eq!(recovered.string_field.as_str(), "test");
    assert_eq!(recovered.fixed32_field.0, 456);
    assert_eq!(recovered.sint32_field.0, -50);
    assert_eq!(recovered.bytes_field.as_ref(), &[10, 20, 30]);
}

#[test]
fn test_unknown_fields_interleaved_with_known() {
    // This tests that unknown fields work correctly when interleaved with known fields
    // by using a message where we only recognize one field in the middle
    // Note: We use MultiWireTypeMessage because it has compatible wire types

    #[derive(Debug, Clone, Default, ProtoMessage)]
    pub struct MiddleFieldOnly {
        // Tag 3 is a string in MultiWireTypeMessage
        #[proto(tag = 3)]
        pub string_field: ProtoString,
        #[proto(unknown)]
        pub _unknown: Bytes,
    }

    let multi = MultiWireTypeMessage {
        varint_field: 42,                                // tag 1 - will be unknown
        fixed64_field: protomon::codec::Fixed64(100),    // tag 2 - will be unknown
        string_field: ProtoString::from("middle_value"), // tag 3 - known
        fixed32_field: protomon::codec::Fixed32(200),    // tag 4 - will be unknown
        sint32_field: protomon::codec::Sint32(-10),      // tag 5 - will be unknown
        bytes_field: protomon::codec::ProtoBytes::from(&[5u8, 6, 7][..]), // tag 6 - will be unknown
    };

    let mut buf = Vec::new();
    multi.encode_message(&mut buf);

    let middle_only =
        MiddleFieldOnly::decode_message(Bytes::from(buf.clone())).expect("decode failed");

    // Tag 3 (string) should be decoded
    assert_eq!(middle_only.string_field.as_str(), "middle_value");

    // Unknown should contain tags 1, 2, 4, 5, 6
    assert!(!middle_only._unknown.is_empty());

    // Re-encode
    let mut buf2 = Vec::new();
    middle_only.encode_message(&mut buf2);

    // Verify round-trip
    let recovered = MultiWireTypeMessage::decode_message(Bytes::from(buf2)).expect("decode failed");
    assert_eq!(recovered.varint_field, 42);
    assert_eq!(recovered.fixed64_field.0, 100);
    assert_eq!(recovered.string_field.as_str(), "middle_value");
    assert_eq!(recovered.fixed32_field.0, 200);
    assert_eq!(recovered.sint32_field.0, -10);
    assert_eq!(recovered.bytes_field.as_ref(), &[5, 6, 7]);
}
