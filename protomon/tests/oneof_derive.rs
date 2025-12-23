//! Integration tests for ProtoOneof derive macro.

use bytes::Bytes;
use protomon::codec::{
    decode_oneof_field, encode_oneof_field, encoded_oneof_field_len, ProtoBytes, ProtoEncode,
    ProtoMessage, ProtoOneof, ProtoString,
};
use protomon::error::DecodeErrorKind;
use protomon::wire::{self, WireType};
use protomon::{ProtoMessage, ProtoOneof};

/// Test oneof using derive macro.
/// Equivalent to:
/// ```protobuf
/// oneof test_oneof {
///     int32 int_value = 1;
///     string string_value = 2;
///     bool bool_value = 3;
///     bytes bytes_value = 4;
/// }
/// ```
#[derive(Debug, Clone, PartialEq, ProtoOneof)]
pub enum TestOneof {
    #[proto(tag = 1)]
    IntValue(i32),
    #[proto(tag = 2)]
    StringValue(ProtoString),
    #[proto(tag = 3)]
    BoolValue(bool),
    #[proto(tag = 4)]
    BytesValue(ProtoBytes),
}

fn roundtrip_oneof(value: TestOneof) {
    // Encode
    let mut buf = Vec::new();
    value.encode_variant(&mut buf);
    assert_eq!(buf.len(), value.encoded_variant_len());

    // Decode
    let mut slice = &buf[..];
    let (wire_type, tag) = wire::decode_key(&mut slice).unwrap();
    let decoded = TestOneof::decode_variant(tag, wire_type, &mut slice, 0)
        .expect("decode failed")
        .expect("tag not recognized");

    assert_eq!(decoded, value);
}

#[test]
fn test_derived_oneof_roundtrip_int() {
    roundtrip_oneof(TestOneof::IntValue(0));
    roundtrip_oneof(TestOneof::IntValue(42));
    roundtrip_oneof(TestOneof::IntValue(-1));
    roundtrip_oneof(TestOneof::IntValue(i32::MAX));
    roundtrip_oneof(TestOneof::IntValue(i32::MIN));
}

#[test]
fn test_derived_oneof_roundtrip_string() {
    roundtrip_oneof(TestOneof::StringValue(ProtoString::from("")));
    roundtrip_oneof(TestOneof::StringValue(ProtoString::from("hello")));
    roundtrip_oneof(TestOneof::StringValue(ProtoString::from("hello world! 🎉")));
}

#[test]
fn test_derived_oneof_roundtrip_bool() {
    roundtrip_oneof(TestOneof::BoolValue(true));
    roundtrip_oneof(TestOneof::BoolValue(false));
}

#[test]
fn test_derived_oneof_roundtrip_bytes() {
    roundtrip_oneof(TestOneof::BytesValue(ProtoBytes::from(&[][..])));
    roundtrip_oneof(TestOneof::BytesValue(ProtoBytes::from(&[1, 2, 3][..])));
    roundtrip_oneof(TestOneof::BytesValue(ProtoBytes::from(&[0u8; 100][..])));
}

#[test]
fn test_derived_oneof_variant_tag() {
    assert_eq!(TestOneof::IntValue(42).variant_tag(), 1);
    assert_eq!(
        TestOneof::StringValue(ProtoString::from("test")).variant_tag(),
        2
    );
    assert_eq!(TestOneof::BoolValue(true).variant_tag(), 3);
    assert_eq!(
        TestOneof::BytesValue(ProtoBytes::from(&[1][..])).variant_tag(),
        4
    );
}

#[test]
fn test_derived_oneof_variant_wire_type() {
    assert_eq!(
        TestOneof::IntValue(42).variant_wire_type(),
        WireType::Varint
    );
    assert_eq!(
        TestOneof::StringValue(ProtoString::from("test")).variant_wire_type(),
        WireType::Len
    );
    assert_eq!(
        TestOneof::BoolValue(true).variant_wire_type(),
        WireType::Varint
    );
    assert_eq!(
        TestOneof::BytesValue(ProtoBytes::from(&[1][..])).variant_wire_type(),
        WireType::Len
    );
}

#[test]
fn test_derived_oneof_unknown_tag() {
    // Encode an int with tag 99 (not in our oneof)
    let mut buf = Vec::new();
    wire::encode_key(WireType::Varint, 99, &mut buf);
    42i32.encode(&mut buf);

    let mut slice = &buf[..];
    let (wire_type, tag) = wire::decode_key(&mut slice).unwrap();
    let result = TestOneof::decode_variant(tag, wire_type, &mut slice, 0).unwrap();

    // Should return None for unknown tag
    assert_eq!(result, None);
}

#[test]
fn test_derived_oneof_option_helpers() {
    let mut oneof: Option<TestOneof> = None;

    // Encode a value
    let mut buf = Vec::new();
    wire::encode_key(WireType::Varint, 1, &mut buf);
    42i32.encode(&mut buf);

    // Decode into Option
    let mut slice = &buf[..];
    let (wire_type, tag) = wire::decode_key(&mut slice).unwrap();
    let matched = decode_oneof_field(&mut oneof, tag, wire_type, &mut slice, 0).unwrap();

    assert!(matched);
    assert_eq!(oneof, Some(TestOneof::IntValue(42)));

    // Test encode_oneof_field
    let mut encoded = Vec::new();
    encode_oneof_field(&oneof, &mut encoded);
    assert_eq!(encoded.len(), encoded_oneof_field_len(&oneof));

    // Decode and verify
    let mut slice = &encoded[..];
    let (wire_type, tag) = wire::decode_key(&mut slice).unwrap();
    let mut decoded: Option<TestOneof> = None;
    decode_oneof_field(&mut decoded, tag, wire_type, &mut slice, 0).unwrap();
    assert_eq!(decoded, Some(TestOneof::IntValue(42)));
}

#[test]
fn test_derived_oneof_last_one_wins() {
    let mut oneof: Option<TestOneof> = None;

    // First, set to IntValue
    let mut buf1 = Vec::new();
    wire::encode_key(WireType::Varint, 1, &mut buf1);
    42i32.encode(&mut buf1);

    let mut slice1 = &buf1[..];
    let (wire_type1, tag1) = wire::decode_key(&mut slice1).unwrap();
    decode_oneof_field(&mut oneof, tag1, wire_type1, &mut slice1, 0).unwrap();
    assert_eq!(oneof, Some(TestOneof::IntValue(42)));

    // Then set to BoolValue - should replace
    let mut buf2 = Vec::new();
    wire::encode_key(WireType::Varint, 3, &mut buf2);
    true.encode(&mut buf2);

    let mut slice2 = &buf2[..];
    let (wire_type2, tag2) = wire::decode_key(&mut slice2).unwrap();
    decode_oneof_field(&mut oneof, tag2, wire_type2, &mut slice2, 0).unwrap();
    assert_eq!(oneof, Some(TestOneof::BoolValue(true))); // Replaced!
}

/// A oneof for use in message tests.
/// Equivalent to:
/// ```protobuf
/// oneof widget {
///     int32 int_field = 2;
///     string string_field = 3;
/// }
/// ```
#[derive(Debug, Clone, PartialEq, ProtoOneof)]
pub enum Widget {
    #[proto(tag = 2)]
    IntField(i32),
    #[proto(tag = 3)]
    StringField(ProtoString),
}

/// Message containing a oneof field.
/// Equivalent to:
/// ```protobuf
/// message MessageWithOneof {
///     string name = 1;
///     oneof widget {
///         int32 int_field = 2;
///         string string_field = 3;
///     }
///     int32 count = 4;
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Default, protomon::ProtoMessage)]
pub struct MessageWithOneof {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(oneof, tags = "2, 3")]
    pub widget: Option<Widget>,
    #[proto(tag = 4)]
    pub count: i32,
}

#[test]
fn test_message_with_oneof_roundtrip() {
    // Test with int variant
    let msg = MessageWithOneof {
        name: ProtoString::from("test"),
        widget: Some(Widget::IntField(42)),
        count: 10,
    };

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);
    assert_eq!(buf.len(), msg.encoded_message_len());

    let decoded = MessageWithOneof::decode_message(Bytes::from(buf)).unwrap();

    assert_eq!(decoded, msg);
}

#[test]
fn test_message_with_oneof_string_variant() {
    let msg = MessageWithOneof {
        name: ProtoString::from("hello"),
        widget: Some(Widget::StringField(ProtoString::from("world"))),
        count: 5,
    };

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);

    let decoded = MessageWithOneof::decode_message(Bytes::from(buf)).unwrap();

    assert_eq!(decoded, msg);
}

#[test]
fn test_message_with_oneof_none() {
    let msg = MessageWithOneof {
        name: ProtoString::from("no widget"),
        widget: None,
        count: 100,
    };

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);

    let decoded = MessageWithOneof::decode_message(Bytes::from(buf)).unwrap();

    assert_eq!(decoded, msg);
    assert!(decoded.widget.is_none());
}

#[test]
fn test_message_with_oneof_last_one_wins() {
    // Manually encode a message with two values for the same oneof
    // (this tests that last-one-wins semantics work at the message level)
    let mut buf = Vec::new();

    // Encode name (tag 1)
    wire::encode_key(WireType::Len, 1, &mut buf);
    ProtoString::from("test").encode(&mut buf);

    // Encode int_field (tag 2) - first oneof value
    wire::encode_key(WireType::Varint, 2, &mut buf);
    42i32.encode(&mut buf);

    // Encode string_field (tag 3) - second oneof value (should win)
    wire::encode_key(WireType::Len, 3, &mut buf);
    ProtoString::from("winner").encode(&mut buf);

    // Encode count (tag 4)
    wire::encode_key(WireType::Varint, 4, &mut buf);
    99i32.encode(&mut buf);

    let decoded = MessageWithOneof::decode_message(Bytes::from(buf)).unwrap();

    assert_eq!(decoded.name, ProtoString::from("test"));
    assert_eq!(
        decoded.widget,
        Some(Widget::StringField(ProtoString::from("winner")))
    );
    assert_eq!(decoded.count, 99);
}

/// Test with a oneof containing a nested message using Box.
#[derive(Debug, Clone, PartialEq, Default, protomon::ProtoMessage)]
pub struct NestedMessage {
    #[proto(tag = 1)]
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, ProtoOneof)]
pub enum OneofWithNested {
    #[proto(tag = 1)]
    Simple(i32),
    #[proto(tag = 2)]
    Nested(Box<NestedMessage>),
}

#[test]
fn test_derived_oneof_with_boxed_message() {
    let nested = NestedMessage { value: 123 };
    let oneof = OneofWithNested::Nested(Box::new(nested));

    // Encode
    let mut buf = Vec::new();
    oneof.encode_variant(&mut buf);

    // Decode
    let mut slice = &buf[..];
    let (wire_type, tag) = wire::decode_key(&mut slice).unwrap();
    let decoded = OneofWithNested::decode_variant(tag, wire_type, &mut slice, 0)
        .unwrap()
        .unwrap();

    match decoded {
        OneofWithNested::Nested(msg) => {
            assert_eq!(msg.value, 123);
        }
        _ => panic!("expected Nested variant"),
    }
}

/// Required oneof enum for testing.
#[derive(Debug, Clone, PartialEq, ProtoOneof)]
pub enum RequiredWidget {
    #[proto(tag = 2)]
    IntValue(i32),
    #[proto(tag = 3)]
    StringValue(ProtoString),
}

impl Default for RequiredWidget {
    fn default() -> Self {
        Self::IntValue(0)
    }
}

/// Message with a required oneof field.
#[derive(Debug, Clone, Default, ProtoMessage)]
pub struct MessageWithRequiredOneof {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(oneof, tags = "2, 3", required)]
    pub widget: RequiredWidget,
}

#[test]
fn test_required_oneof_present_succeeds() {
    // Encode a message with the required oneof present
    let msg = MessageWithRequiredOneof {
        name: ProtoString::from("test"),
        widget: RequiredWidget::IntValue(42),
    };

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);

    // Decode should succeed
    let decoded =
        MessageWithRequiredOneof::decode_message(Bytes::from(buf)).expect("decode should succeed");

    assert_eq!(decoded.name.as_str(), "test");
    assert_eq!(decoded.widget, RequiredWidget::IntValue(42));
}

#[test]
fn test_required_oneof_missing_fails() {
    // Encode a message WITHOUT the required oneof (just the name field)
    let mut buf = Vec::new();
    wire::encode_key(WireType::Len, 1, &mut buf);
    let name = ProtoString::from("test");
    name.encode(&mut buf);
    // Note: We deliberately don't encode the oneof field

    // Decode should fail with MissingRequiredOneof
    let result = MessageWithRequiredOneof::decode_message(Bytes::from(buf));
    match result {
        Err(DecodeErrorKind::MissingRequiredOneof { field }) => {
            assert_eq!(field, "widget");
        }
        Ok(_) => panic!("expected decode to fail for missing required oneof"),
        Err(e) => panic!("expected MissingRequiredOneof error, got: {:?}", e),
    }
}

#[test]
fn test_required_oneof_roundtrip_string_variant() {
    let msg = MessageWithRequiredOneof {
        name: ProtoString::from("hello"),
        widget: RequiredWidget::StringValue(ProtoString::from("world")),
    };

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);

    let decoded =
        MessageWithRequiredOneof::decode_message(Bytes::from(buf)).expect("decode should succeed");

    assert_eq!(decoded.name.as_str(), "hello");
    match decoded.widget {
        RequiredWidget::StringValue(s) => assert_eq!(s.as_str(), "world"),
        _ => panic!("expected StringValue variant"),
    }
}
