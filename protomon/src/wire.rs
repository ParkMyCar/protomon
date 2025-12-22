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
        u64::decode_leb128_buf(buf)?.0
    };

    // The first three bits of the key are the wire type.
    let wire_type = (value & 0b111) as u8;
    let wire_type = WireType::try_from_val(wire_type)?;

    // The remaining bits are the tag / field number.
    let tag = value >> 3;

    Ok((wire_type, tag as u32))
}

/// Decodes the length prefix for a length-delimited field.
#[inline]
pub fn decode_len<B: bytes::Buf>(buf: &mut B) -> Result<usize, DecodeErrorKind> {
    let (len, _) = u64::decode_leb128_buf(buf)?;
    Ok(len as usize)
}

/// Skips over a field value based on its wire type.
///
/// Protobuf supports backwards and fowards compatiblity by skipping fields
/// we don't know about. We "skip" a field by advancing our buffer past it.
#[inline]
pub fn skip_field<B: bytes::Buf>(wire_type: WireType, buf: &mut B) -> Result<(), DecodeErrorKind> {
    let skip_len = match wire_type {
        WireType::Varint => {
            // Read and discard the varint (decode_leb128_buf advances the buffer)
            u64::decode_leb128_buf(buf)?;
            return Ok(());
        }
        WireType::I64 => 8,
        WireType::Len => {
            let len = decode_len(buf)?;
            len
        }
        WireType::I32 => 4,
        WireType::SGroup | WireType::EGroup => {
            return Err(DecodeErrorKind::DeprecatedGroupEncoding);
        }
    };

    if buf.remaining() < skip_len {
        return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
    }
    buf.advance(skip_len);
    Ok(())
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

#[cfg(test)]
mod test {
    use proptest::prelude::*;

    use crate::wire::MINIMUM_TAG_VAL;
    use crate::wire::WireType;
    use crate::wire::decode_key;
    use crate::wire::decode_len;
    use crate::wire::encode_key;
    use crate::wire::skip_field;

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

    #[test]
    fn test_decode_len() {
        // Length 0
        let mut buf = &[0u8][..];
        assert_eq!(decode_len(&mut buf).unwrap(), 0);

        // Length 127 (single byte)
        let mut buf = &[127u8][..];
        assert_eq!(decode_len(&mut buf).unwrap(), 127);

        // Length 128 (two bytes)
        let mut buf = &[0x80, 0x01][..];
        assert_eq!(decode_len(&mut buf).unwrap(), 128);

        // Length 300
        let mut buf = &[0xAC, 0x02][..];
        assert_eq!(decode_len(&mut buf).unwrap(), 300);
    }

    #[test]
    fn test_skip_field_varint() {
        // Skip a 1-byte varint
        let mut buf = &[42u8, 99][..];
        skip_field(WireType::Varint, &mut buf).unwrap();
        assert_eq!(buf, &[99]);

        // Skip a multi-byte varint
        let mut buf = &[0x80, 0x01, 99][..];
        skip_field(WireType::Varint, &mut buf).unwrap();
        assert_eq!(buf, &[99]);
    }

    #[test]
    fn test_skip_field_fixed() {
        // Skip I32
        let mut buf = &[1, 2, 3, 4, 99][..];
        skip_field(WireType::I32, &mut buf).unwrap();
        assert_eq!(buf, &[99]);

        // Skip I64
        let mut buf = &[1, 2, 3, 4, 5, 6, 7, 8, 99][..];
        skip_field(WireType::I64, &mut buf).unwrap();
        assert_eq!(buf, &[99]);
    }

    #[test]
    fn test_skip_field_len() {
        // Skip length-delimited field: length=3, data=[1,2,3]
        let mut buf = &[3, 1, 2, 3, 99][..];
        skip_field(WireType::Len, &mut buf).unwrap();
        assert_eq!(buf, &[99]);

        // Skip empty length-delimited field
        let mut buf = &[0, 99][..];
        skip_field(WireType::Len, &mut buf).unwrap();
        assert_eq!(buf, &[99]);
    }

    #[test]
    fn test_skip_field_groups_error() {
        let mut buf = &[0u8][..];
        assert!(skip_field(WireType::SGroup, &mut buf).is_err());
        assert!(skip_field(WireType::EGroup, &mut buf).is_err());
    }
}
