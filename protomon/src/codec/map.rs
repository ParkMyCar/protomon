//! Protobuf map field support.
//!
//! Maps in protobuf are syntactic sugar for `repeated Entry { K key = 1; V value = 2; }`.
//! Each map entry is encoded as a length-delimited record with two fields.
//!
//! # Wire Format
//!
//! ```text
//! [field_tag, LEN] [entry_len] [key_tag=1, key_wire] [key_value] [value_tag=2, value_wire] [value_value]
//! ```
//!
//! # Valid Key Types
//!
//! Per protobuf spec, valid key types are: integral types, bool, string.
//! NOT valid: float, double, bytes, enum, messages.
//!
//! # Example
//!
//! ```ignore
//! use alloc::collections::BTreeMap;
//! use protomon::ProtoMessage;
//!
//! #[derive(Default, ProtoMessage)]
//! pub struct Config {
//!     #[proto(tag = 1, map)]
//!     pub settings: BTreeMap<String, String>,
//! }
//! ```

use alloc::collections::BTreeMap;

#[cfg(feature = "std")]
use core::hash::Hash;
#[cfg(feature = "std")]
use std::collections::HashMap;

use super::{ProtoDecode, ProtoEncode, ProtoType};
use crate::error::DecodeErrorKind;
use crate::leb128::LebCodec;
use crate::util::CastFrom;
use crate::wire::{self, WireType};

/// Marker trait for types that can be used as protobuf map keys.
///
/// Valid key types per protobuf spec: integral types, bool, string.
/// NOT valid: float, double, bytes, enum, messages.
pub trait ProtoMapKey: ProtoType + ProtoDecode + ProtoEncode + Clone {}

// Implement ProtoMapKey for valid key types
impl ProtoMapKey for i32 {}
impl ProtoMapKey for i64 {}
impl ProtoMapKey for u32 {}
impl ProtoMapKey for u64 {}
impl ProtoMapKey for bool {}
impl ProtoMapKey for super::Sint32 {}
impl ProtoMapKey for super::Sint64 {}
impl ProtoMapKey for super::Fixed32 {}
impl ProtoMapKey for super::Fixed64 {}
impl ProtoMapKey for super::Sfixed32 {}
impl ProtoMapKey for super::Sfixed64 {}
impl ProtoMapKey for super::ProtoString {}
impl ProtoMapKey for alloc::string::String {}

/// Trait for protobuf map fields.
///
/// Provides a unified interface for map fields, whether they use `BTreeMap` or `HashMap`.
/// This trait is used by the derive macro to generate encode/decode code.
pub trait ProtoMap: Default {
    /// Decode a single map entry and insert into the map.
    ///
    /// Implements "last one wins" semantics for duplicate keys.
    fn decode_entry<B: bytes::Buf>(&mut self, buf: &mut B) -> Result<(), DecodeErrorKind>;

    /// Encode all map entries with their field keys.
    fn encode_map<B: bytes::BufMut>(&self, tag: u32, buf: &mut B);

    /// Returns the total encoded length including field keys.
    fn encoded_map_len(&self, tag: u32) -> usize;

    /// Returns the number of entries in the map.
    fn map_len(&self) -> usize;

    /// Returns true if the map is empty.
    fn is_map_empty(&self) -> bool {
        self.map_len() == 0
    }
}

/// Decode a single map entry from the wire format.
///
/// Map entries are encoded as: `<len><key_tag><key_value><value_tag><value_value>`
/// where key_tag = (1 << 3) | key_wire_type and value_tag = (2 << 3) | value_wire_type.
fn decode_map_entry<K, V, B>(buf: &mut B) -> Result<(K, V), DecodeErrorKind>
where
    K: ProtoMapKey + Default,
    V: ProtoType + ProtoDecode + Default,
    B: bytes::Buf,
{
    use bytes::Buf;

    // Read the entry length
    let entry_len = wire::decode_len(buf)?;
    if buf.remaining() < entry_len {
        return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
    }

    // Create a sub-buffer for the entry
    let entry_bytes = buf.copy_to_bytes(entry_len);
    let mut entry_buf = &entry_bytes[..];

    let mut key = K::default();
    let mut value = V::default();

    // Parse the entry fields (key=1, value=2)
    while entry_buf.has_remaining() {
        let (wire_type, tag) = wire::decode_key(&mut entry_buf)?;
        let value_offset = entry_bytes.len() - entry_buf.remaining();

        match tag {
            1 => {
                // Validate wire type matches key type
                if wire_type != K::WIRE_TYPE {
                    return Err(DecodeErrorKind::InvalidWireType {
                        value: wire_type.into_val(),
                    });
                }
                K::decode_into(&mut entry_buf, &mut key, value_offset)?;
            }
            2 => {
                // Validate wire type matches value type
                if wire_type != V::WIRE_TYPE {
                    return Err(DecodeErrorKind::InvalidWireType {
                        value: wire_type.into_val(),
                    });
                }
                V::decode_into(&mut entry_buf, &mut value, value_offset)?;
            }
            _ => {
                // Skip unknown fields within entry
                wire::skip_field(wire_type, &mut entry_buf)?;
            }
        }
    }

    // Per proto3 spec, missing key/value use default values
    Ok((key, value))
}

/// Encode a single map entry to the wire format.
fn encode_map_entry<K, V, B>(key: &K, value: &V, buf: &mut B)
where
    K: ProtoMapKey,
    V: ProtoType + ProtoEncode,
    B: bytes::BufMut,
{
    // Calculate entry length
    let key_field_len = wire::encoded_key_len(1) + key.encoded_len();
    let value_field_len = wire::encoded_key_len(2) + value.encoded_len();
    let entry_len = key_field_len + value_field_len;

    // Write entry length prefix
    u64::cast_from(entry_len).encode_leb128(buf);

    // Write key (tag = 1)
    wire::encode_key(K::WIRE_TYPE, 1, buf);
    key.encode(buf);

    // Write value (tag = 2)
    wire::encode_key(V::WIRE_TYPE, 2, buf);
    value.encode(buf);
}

/// Calculate the encoded length of a map entry (without outer field key).
fn encoded_map_entry_len<K, V>(key: &K, value: &V) -> usize
where
    K: ProtoMapKey,
    V: ProtoType + ProtoEncode,
{
    let key_field_len = wire::encoded_key_len(1) + key.encoded_len();
    let value_field_len = wire::encoded_key_len(2) + value.encoded_len();
    let entry_len = key_field_len + value_field_len;

    // Length prefix + entry content
    u64::cast_from(entry_len).encoded_leb128_len() + entry_len
}

impl<K, V> ProtoType for BTreeMap<K, V>
where
    K: ProtoMapKey,
    V: ProtoType,
{
    // Map entries are length-delimited
    const WIRE_TYPE: WireType = WireType::Len;
}

impl<K, V> ProtoDecode for BTreeMap<K, V>
where
    K: ProtoMapKey + Default + Ord,
    V: ProtoType + ProtoDecode + Default,
{
    /// Decode a single map entry and insert.
    ///
    /// This is called once per entry during message decoding.
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let (key, value) = decode_map_entry::<K, V, B>(buf)?;
        dst.insert(key, value);
        Ok(())
    }
}

impl<K, V> ProtoEncode for BTreeMap<K, V>
where
    K: ProtoMapKey,
    V: ProtoType + ProtoEncode,
{
    /// Encode all entries without field keys (just the entry data).
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        for (key, value) in self {
            encode_map_entry(key, value, buf);
        }
    }

    fn encoded_len(&self) -> usize {
        self.iter().map(|(k, v)| encoded_map_entry_len(k, v)).sum()
    }
}

impl<K, V> ProtoMap for BTreeMap<K, V>
where
    K: ProtoMapKey + Default + Ord,
    V: ProtoType + ProtoDecode + ProtoEncode + Default,
{
    #[inline]
    fn decode_entry<B: bytes::Buf>(&mut self, buf: &mut B) -> Result<(), DecodeErrorKind> {
        let (key, value) = decode_map_entry::<K, V, B>(buf)?;
        self.insert(key, value);
        Ok(())
    }

    fn encode_map<B: bytes::BufMut>(&self, tag: u32, buf: &mut B) {
        for (key, value) in self {
            wire::encode_key(WireType::Len, tag, buf);
            encode_map_entry(key, value, buf);
        }
    }

    fn encoded_map_len(&self, tag: u32) -> usize {
        if self.is_empty() {
            return 0;
        }
        let field_key_len = wire::encoded_key_len(tag);
        self.iter()
            .map(|(k, v)| field_key_len + encoded_map_entry_len(k, v))
            .sum()
    }

    fn map_len(&self) -> usize {
        self.len()
    }
}

#[cfg(feature = "std")]
impl<K, V> ProtoType for HashMap<K, V>
where
    K: ProtoMapKey,
    V: ProtoType,
{
    const WIRE_TYPE: WireType = WireType::Len;
}

#[cfg(feature = "std")]
impl<K, V> ProtoDecode for HashMap<K, V>
where
    K: ProtoMapKey + Default + Hash + Eq,
    V: ProtoType + ProtoDecode + Default,
{
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let (key, value) = decode_map_entry::<K, V, B>(buf)?;
        dst.insert(key, value);
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<K, V> ProtoEncode for HashMap<K, V>
where
    K: ProtoMapKey,
    V: ProtoType + ProtoEncode,
{
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        for (key, value) in self {
            encode_map_entry(key, value, buf);
        }
    }

    fn encoded_len(&self) -> usize {
        self.iter().map(|(k, v)| encoded_map_entry_len(k, v)).sum()
    }
}

#[cfg(feature = "std")]
impl<K, V> ProtoMap for HashMap<K, V>
where
    K: ProtoMapKey + Default + Hash + Eq,
    V: ProtoType + ProtoDecode + ProtoEncode + Default,
{
    #[inline]
    fn decode_entry<B: bytes::Buf>(&mut self, buf: &mut B) -> Result<(), DecodeErrorKind> {
        let (key, value) = decode_map_entry::<K, V, B>(buf)?;
        self.insert(key, value);
        Ok(())
    }

    fn encode_map<B: bytes::BufMut>(&self, tag: u32, buf: &mut B) {
        for (key, value) in self {
            wire::encode_key(WireType::Len, tag, buf);
            encode_map_entry(key, value, buf);
        }
    }

    fn encoded_map_len(&self, tag: u32) -> usize {
        if self.is_empty() {
            return 0;
        }
        let field_key_len = wire::encoded_key_len(tag);
        self.iter()
            .map(|(k, v)| field_key_len + encoded_map_entry_len(k, v))
            .sum()
    }

    fn map_len(&self) -> usize {
        self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;

    #[test]
    fn test_btreemap_roundtrip_string_i32() {
        let mut map: BTreeMap<String, i32> = BTreeMap::new();
        map.insert("apple".into(), 5);
        map.insert("banana".into(), 3);
        map.insert("cherry".into(), 7);

        // Encode with field tag
        let mut buf = Vec::new();
        map.encode_map(1, &mut buf);
        assert_eq!(buf.len(), map.encoded_map_len(1));

        // Decode
        let mut decoded: BTreeMap<String, i32> = BTreeMap::new();
        let mut slice = &buf[..];
        while slice.has_remaining() {
            let (wire_type, tag) = wire::decode_key(&mut slice).unwrap();
            assert_eq!(tag, 1);
            assert_eq!(wire_type, WireType::Len);
            decoded.decode_entry(&mut slice).unwrap();
        }

        assert_eq!(map, decoded);
    }

    #[test]
    fn test_btreemap_roundtrip_i32_string() {
        let mut map: BTreeMap<i32, String> = BTreeMap::new();
        map.insert(1, "one".into());
        map.insert(2, "two".into());
        map.insert(3, "three".into());

        let mut buf = Vec::new();
        map.encode_map(5, &mut buf);

        let mut decoded: BTreeMap<i32, String> = BTreeMap::new();
        let mut slice = &buf[..];
        while slice.has_remaining() {
            let (_, _) = wire::decode_key(&mut slice).unwrap();
            decoded.decode_entry(&mut slice).unwrap();
        }

        assert_eq!(map, decoded);
    }

    #[test]
    fn test_btreemap_empty() {
        let map: BTreeMap<String, i32> = BTreeMap::new();

        let mut buf = Vec::new();
        map.encode_map(1, &mut buf);

        assert!(buf.is_empty());
        assert_eq!(map.encoded_map_len(1), 0);
    }

    #[test]
    fn test_btreemap_duplicate_key_last_wins() {
        // Manually encode two entries with the same key
        let mut buf = Vec::new();

        // First entry: key="test", value=100
        wire::encode_key(WireType::Len, 1, &mut buf);
        encode_map_entry(&String::from("test"), &100i32, &mut buf);

        // Second entry: key="test", value=200 (should win)
        wire::encode_key(WireType::Len, 1, &mut buf);
        encode_map_entry(&String::from("test"), &200i32, &mut buf);

        // Decode
        let mut decoded: BTreeMap<String, i32> = BTreeMap::new();
        let mut slice = &buf[..];
        while slice.has_remaining() {
            let (_, _) = wire::decode_key(&mut slice).unwrap();
            decoded.decode_entry(&mut slice).unwrap();
        }

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded.get("test"), Some(&200));
    }

    #[test]
    fn test_btreemap_bool_key() {
        let mut map: BTreeMap<bool, i32> = BTreeMap::new();
        map.insert(true, 1);
        map.insert(false, 0);

        let mut buf = Vec::new();
        map.encode_map(1, &mut buf);

        let mut decoded: BTreeMap<bool, i32> = BTreeMap::new();
        let mut slice = &buf[..];
        while slice.has_remaining() {
            let (_, _) = wire::decode_key(&mut slice).unwrap();
            decoded.decode_entry(&mut slice).unwrap();
        }

        assert_eq!(map, decoded);
    }

    use bytes::Buf;
}
