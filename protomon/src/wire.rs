//! Wire format for Google's Protocol Buffers, aka [protobuf](https://protobuf.dev).

use core::num::NonZeroU64;

use crate::error::{DecodeError, InvalidKeyReason};
use crate::leb128::LebCodec;
use crate::util::CastFrom;
use crate::util::{likely, unlikely};

/// Minimum value of a protobuf tag.
pub const MINIMUM_TAG_VAL: u32 = 1;
/// Maximum value of a protobuf tag.
pub const MAXIMUM_TAG_VAL: u32 = (1 << 29) - 1;

/// A decoded protobuf field key containing a wire type and tag.
///
/// Packed into a [`NonZeroU64`] to enable register-based returns from
/// [`decode_key`]. In an ideal world this type would use `NonZeroU32` which
/// better resembles a protobuf key, but when used in a Result `rustc` passes
/// the return value on the stack while [`NonZeroU64`] is passed in registers.
///
/// The layout mirrors the protobuf wire format:
/// * Bits 0-2: wire type (0-5)
/// * Bits 3-31: tag/field number (1 to 2^29-1)
///
/// Since tags start at 1, the minimum raw value is 8 (`1 << 3`), guaranteeing
/// the value is always non-zero.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ProtoKey(NonZeroU64);

#[allow(clippy::as_conversions)]
impl ProtoKey {
    /// Creates a new [`ProtoKey`] from a raw key value, validating the wire type and tag.
    ///
    /// Returns an error if the wire type is invalid or the tag is out of range.
    #[inline(always)]
    fn try_from_raw(raw_key: u32) -> Result<Self, DecodeError> {
        // Validate wire type.
        let wire_type_raw = (raw_key & 0b111) as u8;
        if unlikely(wire_type_raw > WireType::MAX_VAL) {
            return Err(DecodeError::invalid_wire_type(wire_type_raw));
        }

        // Validate tag is in valid range.
        let tag = raw_key >> 3;
        if unlikely(tag == 0 || tag > MAXIMUM_TAG_VAL) {
            return Err(DecodeError::invalid_key(InvalidKeyReason::TagOutOfRange));
        }

        // SAFETY: We validated tag >= 1 above raw_key is non-zero.
        Ok(Self(unsafe { NonZeroU64::new_unchecked(raw_key as u64) }))
    }

    /// Returns the [`WireType`] component of this key.
    #[inline(always)]
    pub const fn wire_type(self) -> WireType {
        let raw = (self.0.get() & 0b111) as u8;
        // SAFETY: We validated the wire type during construction.
        unsafe { core::mem::transmute::<u8, WireType>(raw) }
    }

    /// Returns the tag/field number component of this key.
    #[inline(always)]
    pub const fn tag(self) -> u32 {
        (self.0.get() >> 3) as u32
    }

    /// Decomposes this key into its [`WireType`] and tag components.
    #[inline(always)]
    pub const fn into_parts(self) -> (WireType, u32) {
        (self.wire_type(), self.tag())
    }
}

impl core::fmt::Debug for ProtoKey {
    #[cold]
    #[inline(never)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProtoKey")
            .field("wire_type", &self.wire_type())
            .field("tag", &self.tag())
            .finish()
    }
}

/// Encodes the provided tag and wire_type as a protobuf field key.
///
/// Follows the specification from <https://protobuf.dev/programming-guides/encoding>
/// under the "Message Structure" section.
///
/// Hot path for encoding - called for every field in every message.
#[inline(always)]
pub fn encode_key<B: bytes::BufMut>(wire_type: WireType, tag: u32, buf: &mut B) {
    let key = (tag << 3) | u32::cast_from(wire_type.into_val());
    u32::encode_leb128(key, buf);
}

/// Returns the encoded length of a field key (tag + wire type).
///
/// Called frequently during encoded_len() calculations.
#[inline(always)]
pub fn encoded_key_len(tag: u32) -> usize {
    // Wire type is 3 bits, so key = (tag << 3) | wire_type
    // The wire type doesn't affect the length since it only uses 3 bits
    let key = tag << 3;
    key.encoded_leb128_len()
}

/// Decodes the key from a protobuf encoded message.
///
/// Follows the specification from <https://protobuf.dev/programming-guides/encoding>
/// under the "Message Structure" section.
///
/// # Performance
///
/// This is one of the hottest functions in the decode path - it's called for every field
/// in every message. This method signature is very carefully written to ensure arguments
/// and return values are passed entirely in registers instead of on the stack.
///
/// Note: We could annotate this with `#[inline(always)]` but the ~zero stack overhead
/// makes this function very cheap to call, thus we rely on `rustc` or LTO to inline.
#[inline]
pub fn decode_key<B: bytes::Buf>(buf: &mut B) -> Result<ProtoKey, DecodeError> {
    let chunk = buf.chunk();
    let chunk_len = chunk.len();

    // Read a varint from the front of our buffer.
    //
    // N.B. Keys always fit in u32, the max tag value is `2^29-1` and thus the
    // max key value is `(2^29-1) << 3 | 7` which is `u32::MAX`.
    // N.B We hint to the compiler the likely paths for better optimization.
    let value = if unlikely(chunk_len == 0) {
        return Err(DecodeError::invalid_key(InvalidKeyReason::EmptyBuffer));
    } else if likely(chunk[0] < 0x80 || chunk_len >= 5) {
        let (value, bytes_read) = unsafe { u32::decode_leb128(chunk.as_ptr()) }
            .ok_or_else(DecodeError::invalid_varint)?;
        buf.advance(usize::cast_from(bytes_read.get()));
        value
    } else {
        u32::decode_leb128_buf(buf)?.0
    };

    ProtoKey::try_from_raw(value)
}

/// Decodes the length prefix for a length-delimited field.
#[inline(always)]
pub fn decode_len<B: bytes::Buf>(buf: &mut B) -> Result<usize, DecodeError> {
    let chunk = buf.chunk();
    // Fast path, most lengths fit in one byte (< 128).
    if likely(!chunk.is_empty() && chunk[0] < 0x80) {
        let len = usize::cast_from(chunk[0]);
        buf.advance(1);
        Ok(len)
    } else {
        let (len, _) = u64::decode_leb128_buf(buf)?;
        usize::try_from(len).map_err(|_| DecodeError::length_overflow(len))
    }
}

/// Skips over a field value based on its wire type.
///
/// Protobuf supports backwards and fowards compatiblity by skipping fields
/// we don't know about. We "skip" a field by advancing our buffer past it.
///
/// This is on the hot path for message decoding - called for unknown fields
/// and during lazy repeated field iteration.
#[inline(always)]
pub fn skip_field<B: bytes::Buf>(wire_type: WireType, buf: &mut B) -> Result<(), DecodeError> {
    let skip_len = match wire_type {
        WireType::Varint => {
            // Read and discard the varint (decode_leb128_buf advances the buffer)
            u64::decode_leb128_buf(buf)?;
            return Ok(());
        }
        WireType::I64 => 8,
        WireType::Len => decode_len(buf)?,
        WireType::I32 => 4,
        WireType::SGroup | WireType::EGroup => {
            return Err(DecodeError::deprecated_group_encoding());
        }
    };

    if buf.remaining() < skip_len {
        return Err(DecodeError::unexpected_end_of_buffer());
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

#[allow(clippy::as_conversions)]
impl WireType {
    /// Maximum value an [`WireType`] can be.
    const MAX_VAL: u8 = WireType::I32 as u8;

    // Compile-time check that our discriminants are contiguous 0..=MAX_VAL.
    //
    // If someone reorders the enum, this will fail to compile.
    const _DISCRIMINANT_CHECK: () = {
        assert!(WireType::Varint as u8 == 0);
        assert!(WireType::I64 as u8 == 1);
        assert!(WireType::Len as u8 == 2);
        assert!(WireType::SGroup as u8 == 3);
        assert!(WireType::EGroup as u8 == 4);
        assert!(WireType::I32 as u8 == 5);
    };

    /// Try to decode a [`WireType`] from the provided raw value.
    #[inline(always)]
    fn try_from_val(value: u8) -> Result<Self, DecodeError> {
        if value <= Self::MAX_VAL {
            // SAFETY:
            //
            // ValidValue: We checked above that value is within our range.
            // Aligned/Size: WireType and value are both u8
            let wire_type: WireType = unsafe { core::mem::transmute(value) };
            Ok(wire_type)
        } else {
            Err(DecodeError::invalid_wire_type(value))
        }
    }

    /// Return the raw value for this [`WireType`].
    #[inline(always)]
    pub const fn into_val(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for WireType {
    type Error = DecodeError;

    #[inline(always)]
    fn try_from(value: u8) -> Result<Self, DecodeError> {
        WireType::try_from_val(value)
    }
}

#[cfg(test)]
mod test {
    use alloc::vec::Vec;
    use proptest::prelude::*;

    use crate::wire::decode_key;
    use crate::wire::decode_len;
    use crate::wire::encode_key;
    use crate::wire::skip_field;
    use crate::wire::{WireType, MAXIMUM_TAG_VAL, MINIMUM_TAG_VAL};

    #[test]
    fn proptest_key_roundtrips() {
        fn arb_tag() -> impl Strategy<Value = u32> {
            MINIMUM_TAG_VAL..=MAXIMUM_TAG_VAL
        }

        fn arb_wiretype() -> impl Strategy<Value = WireType> {
            (0..5u8).prop_map(|val| WireType::try_from_val(val).expect("known valid"))
        }

        fn test(tag: u32, wire_type: WireType) {
            let mut buf = Vec::with_capacity(16);
            encode_key(wire_type, tag, &mut buf);
            let (rnd_wire_type, rnd_tag) = decode_key(&mut &buf[..]).unwrap().into_parts();

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
