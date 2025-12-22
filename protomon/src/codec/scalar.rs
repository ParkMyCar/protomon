//! Scalar protobuf types and their encoding/decoding implementations.

use super::{ProtoDecode, ProtoEncode, ProtoType};
use crate::error::DecodeErrorKind;
use crate::leb128::LebCodec;
use crate::wire::WireType;

impl ProtoType for u64 {
    const WIRE_TYPE: WireType = WireType::Varint;
}

impl ProtoDecode for u64 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        *dst = u64::decode_leb128_buf(buf).map(|(v, _)| v)?;
        Ok(())
    }
}

impl ProtoEncode for u64 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        self.encode_leb128(buf);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        self.encoded_leb128_len()
    }
}

impl ProtoType for u32 {
    const WIRE_TYPE: WireType = WireType::Varint;
}

impl ProtoDecode for u32 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        *dst = u32::decode_leb128_buf(buf).map(|(v, _)| v)?;
        Ok(())
    }
}

impl ProtoEncode for u32 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        self.encode_leb128(buf);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        self.encoded_leb128_len()
    }
}

impl ProtoType for i64 {
    const WIRE_TYPE: WireType = WireType::Varint;
}

impl ProtoDecode for i64 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        *dst = u64::decode_leb128_buf(buf).map(|(v, _)| v as i64)?;
        Ok(())
    }
}

impl ProtoEncode for i64 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        (*self as u64).encode_leb128(buf);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        (*self as u64).encoded_leb128_len()
    }
}

impl ProtoType for i32 {
    const WIRE_TYPE: WireType = WireType::Varint;
}

impl ProtoDecode for i32 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        // Protobuf int32 is encoded as varint, sign-extended to 64 bits.
        *dst = u64::decode_leb128_buf(buf).map(|(v, _)| v as i32)?;
        Ok(())
    }
}

impl ProtoEncode for i32 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        // Negative values are sign-extended to 64 bits.
        (*self as i64 as u64).encode_leb128(buf);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        (*self as i64 as u64).encoded_leb128_len()
    }
}

impl ProtoType for bool {
    const WIRE_TYPE: WireType = WireType::Varint;
}

impl ProtoDecode for bool {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        *dst = u64::decode_leb128_buf(buf).map(|(v, _)| v != 0)?;
        Ok(())
    }
}

impl ProtoEncode for bool {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        buf.put_u8(*self as u8);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        1
    }
}

#[inline]
pub(crate) const fn zigzag_encode_32(n: i32) -> u32 {
    ((n << 1) ^ (n >> 31)) as u32
}

#[inline]
pub(crate) const fn zigzag_decode_32(n: u32) -> i32 {
    ((n >> 1) as i32) ^ (-((n & 1) as i32))
}

/// Wrapper for protobuf `sint32` (zigzag-encoded signed 32-bit integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct Sint32(pub i32);

impl core::ops::Deref for Sint32 {
    type Target = i32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ProtoType for Sint32 {
    const WIRE_TYPE: WireType = WireType::Varint;
}

impl ProtoDecode for Sint32 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        *dst = Sint32(u32::decode_leb128_buf(buf).map(|(v, _)| zigzag_decode_32(v))?);
        Ok(())
    }
}

impl ProtoEncode for Sint32 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        zigzag_encode_32(self.0).encode_leb128(buf);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        zigzag_encode_32(self.0).encoded_leb128_len()
    }
}

#[inline]
pub(crate) const fn zigzag_encode_64(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

#[inline]
pub(crate) const fn zigzag_decode_64(n: u64) -> i64 {
    ((n >> 1) as i64) ^ (-((n & 1) as i64))
}

/// Wrapper for protobuf `sint64` (zigzag-encoded signed 64-bit integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct Sint64(pub i64);

impl core::ops::Deref for Sint64 {
    type Target = i64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ProtoType for Sint64 {
    const WIRE_TYPE: WireType = WireType::Varint;
}

impl ProtoDecode for Sint64 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        *dst = Sint64(u64::decode_leb128_buf(buf).map(|(v, _)| zigzag_decode_64(v))?);
        Ok(())
    }
}

impl ProtoEncode for Sint64 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        zigzag_encode_64(self.0).encode_leb128(buf);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        zigzag_encode_64(self.0).encoded_leb128_len()
    }
}

/// Wrapper for protobuf `fixed32` (little-endian unsigned 32-bit integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct Fixed32(pub u32);

impl core::ops::Deref for Fixed32 {
    type Target = u32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ProtoType for Fixed32 {
    const WIRE_TYPE: WireType = WireType::I32;
}

impl ProtoDecode for Fixed32 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        if buf.remaining() < 4 {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        *dst = Fixed32(buf.get_u32_le());
        Ok(())
    }
}

impl ProtoEncode for Fixed32 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        buf.put_u32_le(self.0);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        4
    }
}

/// Wrapper for protobuf `fixed64` (little-endian unsigned 64-bit integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct Fixed64(pub u64);

impl core::ops::Deref for Fixed64 {
    type Target = u64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ProtoType for Fixed64 {
    const WIRE_TYPE: WireType = WireType::I64;
}

impl ProtoDecode for Fixed64 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        if buf.remaining() < 8 {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        *dst = Fixed64(buf.get_u64_le());
        Ok(())
    }
}

impl ProtoEncode for Fixed64 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        buf.put_u64_le(self.0);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        8
    }
}

/// Wrapper for protobuf `sfixed32` (little-endian signed 32-bit integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct Sfixed32(pub i32);

impl core::ops::Deref for Sfixed32 {
    type Target = i32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ProtoType for Sfixed32 {
    const WIRE_TYPE: WireType = WireType::I32;
}

impl ProtoDecode for Sfixed32 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        if buf.remaining() < 4 {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        *dst = Sfixed32(buf.get_i32_le());
        Ok(())
    }
}

impl ProtoEncode for Sfixed32 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        buf.put_i32_le(self.0);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        4
    }
}

/// Wrapper for protobuf `sfixed64` (little-endian signed 64-bit integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct Sfixed64(pub i64);

impl core::ops::Deref for Sfixed64 {
    type Target = i64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ProtoType for Sfixed64 {
    const WIRE_TYPE: WireType = WireType::I64;
}

impl ProtoDecode for Sfixed64 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        if buf.remaining() < 8 {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        *dst = Sfixed64(buf.get_i64_le());
        Ok(())
    }
}

impl ProtoEncode for Sfixed64 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        buf.put_i64_le(self.0);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        8
    }
}

impl ProtoType for f32 {
    const WIRE_TYPE: WireType = WireType::I32;
}

impl ProtoDecode for f32 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        if buf.remaining() < 4 {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        *dst = buf.get_f32_le();
        Ok(())
    }
}

impl ProtoEncode for f32 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        buf.put_f32_le(*self);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        4
    }
}

impl ProtoType for f64 {
    const WIRE_TYPE: WireType = WireType::I64;
}

impl ProtoDecode for f64 {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        if buf.remaining() < 8 {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        *dst = buf.get_f64_le();
        Ok(())
    }
}

impl ProtoEncode for f64 {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        buf.put_f64_le(*self);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T: ProtoEncode + ProtoDecode + PartialEq + std::fmt::Debug + Default>(value: T) {
        let mut buf = Vec::new();
        value.encode(&mut buf);
        assert_eq!(buf.len(), value.encoded_len());
        let mut decoded = T::default();
        T::decode_into(&mut &buf[..], &mut decoded, 0).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn test_varint_roundtrip() {
        roundtrip(0u32);
        roundtrip(127u32);
        roundtrip(128u32);
        roundtrip(u32::MAX);

        roundtrip(0u64);
        roundtrip(u64::MAX);

        roundtrip(0i32);
        roundtrip(-1i32);
        roundtrip(i32::MIN);
        roundtrip(i32::MAX);

        roundtrip(0i64);
        roundtrip(-1i64);
        roundtrip(i64::MIN);
        roundtrip(i64::MAX);

        roundtrip(true);
        roundtrip(false);
    }

    #[test]
    fn test_zigzag_roundtrip() {
        roundtrip(Sint32(0));
        roundtrip(Sint32(-1));
        roundtrip(Sint32(1));
        roundtrip(Sint32(i32::MIN));
        roundtrip(Sint32(i32::MAX));

        roundtrip(Sint64(0));
        roundtrip(Sint64(-1));
        roundtrip(Sint64(1));
        roundtrip(Sint64(i64::MIN));
        roundtrip(Sint64(i64::MAX));
    }

    #[test]
    fn test_zigzag_encoding() {
        // From protobuf spec
        assert_eq!(zigzag_encode_32(0), 0);
        assert_eq!(zigzag_encode_32(-1), 1);
        assert_eq!(zigzag_encode_32(1), 2);
        assert_eq!(zigzag_encode_32(-2), 3);
        assert_eq!(zigzag_encode_32(2147483647), 4294967294);
        assert_eq!(zigzag_encode_32(-2147483648), 4294967295);
    }

    #[test]
    fn test_fixed_roundtrip() {
        roundtrip(Fixed32(0));
        roundtrip(Fixed32(u32::MAX));

        roundtrip(Fixed64(0));
        roundtrip(Fixed64(u64::MAX));

        roundtrip(Sfixed32(0));
        roundtrip(Sfixed32(i32::MIN));
        roundtrip(Sfixed32(i32::MAX));

        roundtrip(Sfixed64(0));
        roundtrip(Sfixed64(i64::MIN));
        roundtrip(Sfixed64(i64::MAX));
    }

    #[test]
    fn test_float_roundtrip() {
        roundtrip(0.0f32);
        roundtrip(1.0f32);
        roundtrip(-1.0f32);
        roundtrip(f32::MIN);
        roundtrip(f32::MAX);

        roundtrip(0.0f64);
        roundtrip(1.0f64);
        roundtrip(-1.0f64);
        roundtrip(f64::MIN);
        roundtrip(f64::MAX);
    }
}
