//! Wire format for Google's Protocol Buffers, aka [protobuf](https://protobuf.dev).

use crate::error::DecodeErrorKind;
use crate::leb128::LebCodec;
use crate::util::{likely, unlikely};

/// Minimum value of a protobuf tag.
pub const MINIMUM_TAG_VAL: u32 = 1;
/// Maximum value of a protobuf tag.
pub const MAXIMUM_TAG_VAL: u32 = (1 << 29) - 1;

/// Encodes the provided tag and wire_type as a protobuf field key.
///
/// Follows the specification from <https://protobuf.dev/programming-guides/encoding>
/// under the "Message Structure" section.
#[inline]
pub fn encode_key<B: bytes::BufMut>(wire_type: WireType, tag: u32, buf: &mut B) {
    let key = (tag << 3) | wire_type as u32;
    u32::encode_leb128(key, buf);
}

/// Decodes the key from a protobuf encoded message.
///
/// Follows the specification from <https://protobuf.dev/programming-guides/encoding>
/// under the "Message Structure" section.
#[inline(always)]
pub fn decode_key<B: bytes::Buf>(buf: &mut B) -> Result<(WireType, u32), DecodeErrorKind> {
    let chunk = buf.chunk();
    let chunk_len = chunk.len();

    // Read a varint from the front of our buffer.
    //
    // Note: We hint to the compiler the likely paths for better optimization.
    let value = if unlikely(chunk_len == 0) {
        return Err(DecodeErrorKind::InvalidKey {
            reason: "empty buffer",
        });
    } else if likely(chunk[0] < 0x80 || chunk_len > 10) {
        let (value, bytes_read) = unsafe { u64::decode_leb128(chunk)? };
        buf.advance(bytes_read as usize);
        value
    } else {
        u64::decode_leb128_buf(buf)?
    };

    // The first three bits of the key are the wire type.
    let wire_type = (value & 0b111) as u8;
    let wire_type = WireType::try_from_val(wire_type)?;

    // The remaining bits are the tag / field number.
    let tag = value >> 3;

    Ok((wire_type, tag as u32))
}

/// Denotes the type of a field in an encoded protobuf message.
///
/// Protobuf messages are a series of key-value pairs. When encoded each key-value pair
/// is turned into a record consisting of a field number, a [`WireType`], and a payload.
/// The [`WireType`] indicates how large the proceeding payload is.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
#[allow(dead_code)] // We construct this enum via transmute.
pub enum WireType {
    /// Variable length integer.
    ///
    /// Used for: `int32`, `int64`, `uint32`, `uint64`, `sint32`, `sint64`, `bool`, `enum`.
    Varint = 0,
    /// 64-bit integer.
    ///
    /// Used for: `fixed64`, `sfixed64`, `double`.
    I64 = 1,
    /// Variable length field.
    ///
    /// Used for: `string`, `bytes`, `message`, packed `repeated` fields.
    Len = 2,
    /// Group start (deprecated).
    SGroup = 3,
    /// Group end (deprecated).
    EGroup = 4,
    /// 32-bit integer.
    ///
    /// Used for: `fixed32`, `sfixed32`, `float`.
    I32 = 5,
}

// N.B. It's not super important that these are the same size, but keeping them as such
// allows the compiler to make as many optimizations as possible.
crate::util::assert_eq_size!(WireType, Result<WireType, ()>);

impl WireType {
    /// Maximum value an [`WireType`] can be.
    const MAX_VAL: u8 = WireType::I32 as u8;

    /// Try to decode a [`WireType`] from the provided raw value.
    #[inline(always)]
    const fn try_from_val(value: u8) -> Result<Self, DecodeErrorKind> {
        if value <= Self::MAX_VAL {
            // SAFETY:
            //
            // ValidValue: We checked above that value is within our range.
            // Aligned/Size: WireType and value are both u8
            let wire_type: WireType = unsafe { core::mem::transmute(value) };
            Ok(wire_type)
        } else {
            Err(DecodeErrorKind::InvalidWireType { value })
        }
    }

    /// Return the raw value for this [`WireType`].
    #[inline(always)]
    pub const fn into_val(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for WireType {
    type Error = DecodeErrorKind;

    #[inline(always)]
    fn try_from(value: u8) -> Result<Self, DecodeErrorKind> {
        WireType::try_from_val(value)
    }
}

pub trait Decode {
    type Target<'a>
    where
        Self: 'a;

    fn decode_slice<'s>(s: &'s [u8]) -> Self::Target<'s>;
}

impl Decode for str {
    type Target<'a>
        = &'a str
    where
        Self: 'a;

    fn decode_slice<'s>(s: &'s [u8]) -> Self::Target<'s> {
        unsafe { std::str::from_utf8_unchecked(s) }
    }
}

impl Decode for String {
    type Target<'a>
        = String
    where
        Self: 'a;

    fn decode_slice<'s>(s: &'s [u8]) -> String {
        unsafe { std::str::from_utf8_unchecked(s).to_string() }
    }
}

#[cfg(test)]
mod test {
    use proptest::prelude::*;

    use crate::wire::MINIMUM_TAG_VAL;
    use crate::wire::WireType;
    use crate::wire::decode_key;
    use crate::wire::encode_key;

    #[test]
    fn proptest_key_roundtrips() {
        fn arb_tag() -> impl Strategy<Value = u32> {
            MINIMUM_TAG_VAL..=MINIMUM_TAG_VAL
        }

        fn arb_wiretype() -> impl Strategy<Value = WireType> {
            (0..5u8).prop_map(|val| WireType::try_from_val(val).expect("known valid"))
        }

        fn test(tag: u32, wire_type: WireType) {
            let mut buf = Vec::with_capacity(16);
            encode_key(wire_type, tag, &mut buf);
            let (rnd_wire_type, rnd_tag) = decode_key(&mut &buf[..]).unwrap();

            assert_eq!(tag, rnd_tag);
            assert_eq!(wire_type, rnd_wire_type);
        }

        let strat = (arb_tag(), arb_wiretype());
        proptest!(|((tag, wire_type) in strat)| test(tag, wire_type))
    }

    #[test]
    fn test_all_valid_values() {
        // N.B. We do not use proptest here because the range of values is
        // small enough, but also this way we get coverage via Miri for any
        // unsafe shenanigans.
        for i in u8::MIN..u8::MAX {
            let wire_type = WireType::try_from_val(i);
            match (i, wire_type) {
                (0, Ok(WireType::Varint))
                | (1, Ok(WireType::I64))
                | (2, Ok(WireType::Len))
                | (3, Ok(WireType::SGroup))
                | (4, Ok(WireType::EGroup))
                | (5, Ok(WireType::I32)) => (),
                (_, Err(_)) => (),
                other => panic!("unexpected value {other:?}"),
            }
        }
    }
}
