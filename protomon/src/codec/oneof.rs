//! Oneof field support for protobuf.
//!
//! Protobuf oneofs map naturally to Rust enums. This module provides the
//! [`ProtoOneof`] trait for oneof encode/decode.
//!
//! # Wire Format
//!
//! Oneofs have no wire representation of their own. Each variant is encoded
//! as a regular field with its own tag. The mutual exclusivity is enforced
//! at the Rust type level, and "last one wins" semantics apply during decode.
//!
//! # Example
//!
//! ```ignore
//! // Given protobuf:
//! // message Foo {
//! //   oneof widget {
//! //     int32 quux = 1;
//! //     string bar = 2;
//! //   }
//! // }
//!
//! #[derive(ProtoOneof)]
//! pub enum Widget {
//!     #[proto(tag = 1)]
//!     Quux(i32),
//!     #[proto(tag = 2)]
//!     Bar(ProtoString),
//! }
//!
//! // In the message:
//! pub struct Foo {
//!     pub widget: Option<Widget>,
//! }
//! ```

use crate::error::DecodeError;
use crate::wire::WireType;

/// Trait for protobuf oneof types.
///
/// Oneofs are represented as Rust enums where each variant corresponds to
/// a possible field. Only one field can be set at a time.
///
/// # Wire Format
///
/// Oneof fields are encoded as regular fields - the oneof itself has no
/// wire representation. The mutual exclusivity is enforced at the type level.
///
/// When decoding, if multiple fields from the same oneof appear in the wire
/// data, the last one wins (per protobuf spec).
pub trait ProtoOneof: Sized {
    /// Decode a oneof variant from the given tag and buffer.
    ///
    /// # Returns
    /// - `Ok(Some(variant))` if the tag matches a variant in this oneof
    /// - `Ok(None)` if the tag doesn't match any variant (unknown field)
    /// - `Err(...)` if decoding fails
    ///
    /// # Parameters
    /// - `tag`: The field tag from the wire
    /// - `wire_type`: The wire type from the key
    /// - `buf`: Buffer positioned at the value (after key)
    /// - `offset`: Byte offset of this value in the message buffer
    fn decode_variant<B: bytes::Buf>(
        tag: u32,
        wire_type: WireType,
        buf: &mut B,
        offset: usize,
    ) -> Result<Option<Self>, DecodeError>;

    /// Encode this oneof variant to the buffer.
    ///
    /// This writes the complete field including the key (tag + wire type).
    fn encode_variant<B: bytes::BufMut>(&self, buf: &mut B);

    /// Returns the encoded length of this variant (including field key).
    fn encoded_variant_len(&self) -> usize;

    /// Returns the tag of the currently active variant.
    fn variant_tag(&self) -> u32;

    /// Returns the wire type of the currently active variant.
    fn variant_wire_type(&self) -> WireType;
}

/// Helper to decode a oneof field into Option<T>.
///
/// This is used by generated code to handle oneof fields in messages.
/// It implements "last one wins" semantics by replacing any existing value.
#[inline]
pub fn decode_oneof_field<T: ProtoOneof, B: bytes::Buf>(
    dst: &mut Option<T>,
    tag: u32,
    wire_type: WireType,
    buf: &mut B,
    offset: usize,
) -> Result<bool, DecodeError> {
    match T::decode_variant(tag, wire_type, buf, offset)? {
        Some(value) => {
            *dst = Some(value);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Helper to encode an Option<T> oneof field.
#[inline]
pub fn encode_oneof_field<T: ProtoOneof, B: bytes::BufMut>(field: &Option<T>, buf: &mut B) {
    if let Some(ref value) = field {
        value.encode_variant(buf);
    }
}

/// Helper to get the encoded length of an Option<T> oneof field.
#[inline]
pub fn encoded_oneof_field_len<T: ProtoOneof>(field: &Option<T>) -> usize {
    match field {
        Some(value) => value.encoded_variant_len(),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{ProtoDecode, ProtoEncode, ProtoString, ProtoType};
    use crate::wire;
    use alloc::vec::Vec;

    /// Example oneof enum for testing.
    /// Equivalent to:
    /// ```protobuf
    /// oneof widget {
    ///     int32 quux = 1;
    ///     string bar = 2;
    ///     bool flag = 3;
    /// }
    /// ```
    #[derive(Debug, Clone, PartialEq)]
    enum Widget {
        Quux(i32),
        Bar(ProtoString),
        Flag(bool),
    }

    impl ProtoOneof for Widget {
        fn decode_variant<B: bytes::Buf>(
            tag: u32,
            wire_type: WireType,
            buf: &mut B,
            offset: usize,
        ) -> Result<Option<Self>, DecodeError> {
            match tag {
                1 => {
                    if wire_type != <i32 as ProtoType>::WIRE_TYPE {
                        return Err(DecodeError::invalid_wire_type(wire_type.into_val()));
                    }
                    let mut value = i32::default();
                    i32::decode_into(buf, &mut value, offset)?;
                    Ok(Some(Widget::Quux(value)))
                }
                2 => {
                    if wire_type != <ProtoString as ProtoType>::WIRE_TYPE {
                        return Err(DecodeError::invalid_wire_type(wire_type.into_val()));
                    }
                    let mut value = ProtoString::default();
                    ProtoString::decode_into(buf, &mut value, offset)?;
                    Ok(Some(Widget::Bar(value)))
                }
                3 => {
                    if wire_type != <bool as ProtoType>::WIRE_TYPE {
                        return Err(DecodeError::invalid_wire_type(wire_type.into_val()));
                    }
                    let mut value = bool::default();
                    bool::decode_into(buf, &mut value, offset)?;
                    Ok(Some(Widget::Flag(value)))
                }
                _ => Ok(None),
            }
        }

        fn encode_variant<B: bytes::BufMut>(&self, buf: &mut B) {
            match self {
                Widget::Quux(value) => {
                    wire::encode_key(<i32 as ProtoType>::WIRE_TYPE, 1, buf);
                    value.encode(buf);
                }
                Widget::Bar(value) => {
                    wire::encode_key(<ProtoString as ProtoType>::WIRE_TYPE, 2, buf);
                    value.encode(buf);
                }
                Widget::Flag(value) => {
                    wire::encode_key(<bool as ProtoType>::WIRE_TYPE, 3, buf);
                    value.encode(buf);
                }
            }
        }

        fn encoded_variant_len(&self) -> usize {
            match self {
                Widget::Quux(value) => wire::encoded_key_len(1) + value.encoded_len(),
                Widget::Bar(value) => wire::encoded_key_len(2) + value.encoded_len(),
                Widget::Flag(value) => wire::encoded_key_len(3) + value.encoded_len(),
            }
        }

        fn variant_tag(&self) -> u32 {
            match self {
                Widget::Quux(_) => 1,
                Widget::Bar(_) => 2,
                Widget::Flag(_) => 3,
            }
        }

        fn variant_wire_type(&self) -> WireType {
            match self {
                Widget::Quux(_) => <i32 as ProtoType>::WIRE_TYPE,
                Widget::Bar(_) => <ProtoString as ProtoType>::WIRE_TYPE,
                Widget::Flag(_) => <bool as ProtoType>::WIRE_TYPE,
            }
        }
    }

    fn roundtrip_oneof(widget: Widget) {
        // Encode
        let mut buf = Vec::new();
        widget.encode_variant(&mut buf);
        assert_eq!(buf.len(), widget.encoded_variant_len());

        // Decode
        let mut slice = &buf[..];
        let (wire_type, tag) = wire::decode_key(&mut slice).unwrap().into_parts();
        let decoded = Widget::decode_variant(tag, wire_type, &mut slice, 0).expect("decode failed");

        assert_eq!(decoded, Some(widget));
    }

    #[test]
    fn test_oneof_roundtrip_int() {
        roundtrip_oneof(Widget::Quux(42));
        roundtrip_oneof(Widget::Quux(0));
        roundtrip_oneof(Widget::Quux(-1));
        roundtrip_oneof(Widget::Quux(i32::MAX));
        roundtrip_oneof(Widget::Quux(i32::MIN));
    }

    #[test]
    fn test_oneof_roundtrip_string() {
        roundtrip_oneof(Widget::Bar(ProtoString::from("")));
        roundtrip_oneof(Widget::Bar(ProtoString::from("hello")));
        roundtrip_oneof(Widget::Bar(ProtoString::from("hello world! 🎉")));
    }

    #[test]
    fn test_oneof_roundtrip_bool() {
        roundtrip_oneof(Widget::Flag(true));
        roundtrip_oneof(Widget::Flag(false));
    }

    #[test]
    fn test_oneof_unknown_tag() {
        // Encode an int with tag 99 (not in our oneof)
        let mut buf = Vec::new();
        wire::encode_key(WireType::Varint, 99, &mut buf);
        42i32.encode(&mut buf);

        let mut slice = &buf[..];
        let (wire_type, tag) = wire::decode_key(&mut slice).unwrap().into_parts();
        let result = Widget::decode_variant(tag, wire_type, &mut slice, 0).unwrap();

        // Should return None for unknown tag
        assert_eq!(result, None);
    }

    #[test]
    fn test_oneof_option_helper() {
        let mut widget: Option<Widget> = None;

        // Encode a value
        let mut buf = Vec::new();
        wire::encode_key(WireType::Varint, 1, &mut buf);
        42i32.encode(&mut buf);

        // Decode into Option
        let mut slice = &buf[..];
        let (wire_type, tag) = wire::decode_key(&mut slice).unwrap().into_parts();
        let matched = decode_oneof_field(&mut widget, tag, wire_type, &mut slice, 0).unwrap();

        assert!(matched);
        assert_eq!(widget, Some(Widget::Quux(42)));

        // Encode another value (last one wins)
        let mut buf2 = Vec::new();
        wire::encode_key(WireType::Varint, 3, &mut buf2);
        true.encode(&mut buf2);

        let mut slice2 = &buf2[..];
        let (wire_type2, tag2) = wire::decode_key(&mut slice2).unwrap().into_parts();
        let matched2 = decode_oneof_field(&mut widget, tag2, wire_type2, &mut slice2, 0).unwrap();

        assert!(matched2);
        assert_eq!(widget, Some(Widget::Flag(true))); // Replaced!
    }

    #[test]
    fn test_oneof_encode_option() {
        let widget: Option<Widget> = Some(Widget::Bar(ProtoString::from("test")));

        let mut buf = Vec::new();
        encode_oneof_field(&widget, &mut buf);

        assert_eq!(buf.len(), encoded_oneof_field_len(&widget));

        // Decode and verify
        let mut slice = &buf[..];
        let (wire_type, tag) = wire::decode_key(&mut slice).unwrap().into_parts();
        assert_eq!(tag, 2);
        assert_eq!(wire_type, WireType::Len);
    }

    #[test]
    fn test_oneof_encode_none() {
        let widget: Option<Widget> = None;

        let mut buf = Vec::new();
        encode_oneof_field(&widget, &mut buf);

        assert!(buf.is_empty());
        assert_eq!(encoded_oneof_field_len(&widget), 0);
    }
}
