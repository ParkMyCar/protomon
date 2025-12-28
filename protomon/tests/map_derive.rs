//! Integration tests for map field support with derive macro.

use bytes::Bytes;
use protomon::codec::{ProtoEncode, ProtoMessage, ProtoString};
use protomon::ProtoMessage;
use std::collections::BTreeMap;

/// Basic message with a string-to-i32 map.
#[derive(Debug, Clone, PartialEq, Default, ProtoMessage)]
pub struct StringToIntMap {
    #[proto(tag = 1, map)]
    pub entries: BTreeMap<String, i32>,
}

/// Message with an i32-to-string map (reversed key/value types).
#[derive(Debug, Clone, PartialEq, Default, ProtoMessage)]
pub struct IntToStringMap {
    #[proto(tag = 1, map)]
    pub entries: BTreeMap<i32, String>,
}

/// Message with multiple map fields.
#[derive(Debug, Clone, PartialEq, Default, ProtoMessage)]
pub struct MultipleMapFields {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(tag = 2, map)]
    pub settings: BTreeMap<String, String>,
    #[proto(tag = 3, map)]
    pub counts: BTreeMap<i32, i64>,
}

/// Message with a bool key map.
#[derive(Debug, Clone, PartialEq, Default, ProtoMessage)]
pub struct BoolKeyMap {
    #[proto(tag = 1, map)]
    pub flags: BTreeMap<bool, i32>,
}

#[test]
fn test_string_to_int_map_roundtrip() {
    let mut msg = StringToIntMap::default();
    msg.entries.insert("apple".into(), 5);
    msg.entries.insert("banana".into(), 3);
    msg.entries.insert("cherry".into(), 7);

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);
    assert_eq!(buf.len(), msg.encoded_message_len());

    let decoded = StringToIntMap::decode_message(Bytes::from(buf)).expect("decode failed");
    assert_eq!(decoded, msg);
}

#[test]
fn test_int_to_string_map_roundtrip() {
    let mut msg = IntToStringMap::default();
    msg.entries.insert(1, "one".into());
    msg.entries.insert(2, "two".into());
    msg.entries.insert(3, "three".into());

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);
    assert_eq!(buf.len(), msg.encoded_message_len());

    let decoded = IntToStringMap::decode_message(Bytes::from(buf)).expect("decode failed");
    assert_eq!(decoded, msg);
}

#[test]
fn test_empty_map() {
    let msg = StringToIntMap::default();
    assert!(msg.entries.is_empty());

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);

    // Empty map should produce empty message
    assert!(buf.is_empty());
    assert_eq!(msg.encoded_message_len(), 0);

    let decoded = StringToIntMap::decode_message(Bytes::from(buf)).expect("decode failed");
    assert!(decoded.entries.is_empty());
}

#[test]
fn test_multiple_map_fields() {
    let mut msg = MultipleMapFields {
        name: ProtoString::from("config"),
        ..Default::default()
    };
    msg.settings.insert("key1".into(), "value1".into());
    msg.settings.insert("key2".into(), "value2".into());
    msg.counts.insert(1, 100);
    msg.counts.insert(2, 200);

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);
    assert_eq!(buf.len(), msg.encoded_message_len());

    let decoded = MultipleMapFields::decode_message(Bytes::from(buf)).expect("decode failed");
    assert_eq!(decoded, msg);
}

#[test]
fn test_bool_key_map() {
    let mut msg = BoolKeyMap::default();
    msg.flags.insert(true, 1);
    msg.flags.insert(false, 0);

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);
    assert_eq!(buf.len(), msg.encoded_message_len());

    let decoded = BoolKeyMap::decode_message(Bytes::from(buf)).expect("decode failed");
    assert_eq!(decoded, msg);
}

#[test]
fn test_duplicate_key_last_wins() {
    use protomon::wire::{self, WireType};

    // Manually encode a message with duplicate map entries
    let mut buf = Vec::new();

    // First entry: key="test", value=100
    wire::encode_key(WireType::Len, 1, &mut buf);
    let mut map1: BTreeMap<String, i32> = BTreeMap::new();
    map1.insert("test".into(), 100);
    // Encode just the entry (not the field key again)
    let mut entry_buf = Vec::new();
    encode_single_entry(&"test".to_string(), &100i32, &mut entry_buf);
    buf.extend_from_slice(&entry_buf);

    // Second entry: key="test", value=200 (should win)
    wire::encode_key(WireType::Len, 1, &mut buf);
    let mut entry_buf2 = Vec::new();
    encode_single_entry(&"test".to_string(), &200i32, &mut entry_buf2);
    buf.extend_from_slice(&entry_buf2);

    // Decode
    let decoded = StringToIntMap::decode_message(Bytes::from(buf)).expect("decode failed");

    // Last one wins
    assert_eq!(decoded.entries.len(), 1);
    assert_eq!(decoded.entries.get("test"), Some(&200));
}

/// Helper to encode a single map entry for testing.
fn encode_single_entry<K: ProtoEncode, V: ProtoEncode>(key: &K, value: &V, buf: &mut Vec<u8>) {
    use protomon::codec::ProtoType;
    use protomon::leb128::LebCodec;
    use protomon::wire;

    let key_field_len = wire::encoded_key_len(1) + key.encoded_len();
    let value_field_len = wire::encoded_key_len(2) + value.encoded_len();
    let entry_len = key_field_len + value_field_len;

    (entry_len as u64).encode_leb128(buf);
    wire::encode_key(<K as ProtoType>::WIRE_TYPE, 1, buf);
    key.encode(buf);
    wire::encode_key(<V as ProtoType>::WIRE_TYPE, 2, buf);
    value.encode(buf);
}

#[test]
fn test_large_map() {
    let mut msg = IntToStringMap::default();
    for i in 0..100 {
        msg.entries.insert(i, format!("value_{}", i));
    }

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);
    assert_eq!(buf.len(), msg.encoded_message_len());

    let decoded = IntToStringMap::decode_message(Bytes::from(buf)).expect("decode failed");
    assert_eq!(decoded, msg);
    assert_eq!(decoded.entries.len(), 100);
}

#[test]
fn test_map_with_empty_key() {
    let mut msg = StringToIntMap::default();
    msg.entries.insert("".into(), 42); // Empty string key

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);

    let decoded = StringToIntMap::decode_message(Bytes::from(buf)).expect("decode failed");
    assert_eq!(decoded.entries.get(""), Some(&42));
}

#[test]
fn test_map_with_empty_value() {
    let mut msg = IntToStringMap::default();
    msg.entries.insert(1, "".into()); // Empty string value

    let mut buf = Vec::new();
    msg.encode_message(&mut buf);

    let decoded = IntToStringMap::decode_message(Bytes::from(buf)).expect("decode failed");
    assert_eq!(decoded.entries.get(&1), Some(&String::from("")));
}
