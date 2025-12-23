//! Length-delimited protobuf types (bytes, string).

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use super::{ProtoDecode, ProtoEncode, ProtoType};
use crate::error::DecodeErrorKind;
use crate::leb128::LebCodec;
use crate::wire::WireType;

/// Wrapper for protobuf `bytes` field (length-delimited raw bytes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProtoBytes(pub bytes::Bytes);

impl core::ops::Deref for ProtoBytes {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<bytes::Bytes> for ProtoBytes {
    fn from(b: bytes::Bytes) -> Self {
        ProtoBytes(b)
    }
}

impl From<&[u8]> for ProtoBytes {
    fn from(b: &[u8]) -> Self {
        ProtoBytes(bytes::Bytes::copy_from_slice(b))
    }
}

impl ProtoType for ProtoBytes {
    const WIRE_TYPE: WireType = WireType::Len;
}

impl ProtoDecode for ProtoBytes {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let len = crate::wire::decode_len(buf)?;
        if buf.remaining() < len {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        *dst = ProtoBytes(buf.copy_to_bytes(len));
        Ok(())
    }
}

impl ProtoEncode for ProtoBytes {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        (self.0.len() as u64).encode_leb128(buf);
        buf.put_slice(&self.0);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        (self.0.len() as u64).encoded_leb128_len() + self.0.len()
    }
}

/// Wrapper for protobuf `string` field (length-delimited UTF-8 string).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProtoString(bytes::Bytes);

impl ProtoString {
    /// Returns the string as a `&str`.
    ///
    /// # Safety
    /// The bytes are validated as UTF-8 during decode, so this is safe.
    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.0) }
    }

    /// Returns the underlying bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Consumes the ProtoString and returns the underlying Bytes.
    pub fn into_bytes(self) -> bytes::Bytes {
        self.0
    }
}

impl core::ops::Deref for ProtoString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl From<&str> for ProtoString {
    fn from(s: &str) -> Self {
        ProtoString(bytes::Bytes::copy_from_slice(s.as_bytes()))
    }
}

#[cfg(feature = "alloc")]
impl From<String> for ProtoString {
    fn from(s: String) -> Self {
        ProtoString(bytes::Bytes::from(s))
    }
}

impl ProtoType for ProtoString {
    const WIRE_TYPE: WireType = WireType::Len;
}

impl ProtoDecode for ProtoString {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let len = crate::wire::decode_len(buf)?;
        if buf.remaining() < len {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        let data = buf.copy_to_bytes(len);

        // Validate UTF-8.
        if core::str::from_utf8(&data).is_err() {
            return Err(DecodeErrorKind::InvalidUtf8);
        }
        *dst = ProtoString(data);

        Ok(())
    }
}

impl ProtoEncode for ProtoString {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        (self.0.len() as u64).encode_leb128(buf);
        buf.put_slice(&self.0);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        (self.0.len() as u64).encoded_leb128_len() + self.0.len()
    }
}

#[cfg(feature = "alloc")]
impl ProtoType for String {
    const WIRE_TYPE: WireType = WireType::Len;
}

#[cfg(feature = "alloc")]
impl ProtoDecode for String {
    #[inline(always)]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let len = crate::wire::decode_len(buf)?;
        if buf.remaining() < len {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }

        // Use a guard to safely handle uninitialized memory.
        //
        // If copy_to_slice panics, the guard ensures we don't expose uninitialized bytes.
        struct Guard<'a> {
            buf: &'a mut Vec<u8>,
        }
        impl Drop for Guard<'_> {
            // SAFETY: If the guard is dropped, e.g. a panic, clear the Vec so we don't
            // expose non-valid UTF-8.
            fn drop(&mut self) {
                self.buf.clear();
            }
        }

        // SAFETY: We wrap the mutable reference in a guard which automatically clears
        // the buffer if we have non-valid UTF-8.
        let str_buf = unsafe { dst.as_mut_vec() };
        let guard = Guard { buf: str_buf };

        // It's possible for a field to show up multiple times in an encoded payload. We
        // clear the buffer incase we've seen this field before.
        guard.buf.clear();
        guard.buf.reserve(len);
        // SAFETY: reserve guarantees capacity >= len, guard clears on panic/invalid UTF-8.
        unsafe { guard.buf.set_len(len) };

        buf.copy_to_slice(&mut guard.buf[..len]);

        // Validate that the buffer is UTF-8.
        match core::str::from_utf8(&guard.buf[..]) {
            // We have valid UTF-8! We can forget the guard.
            Ok(_) => core::mem::forget(guard),
            // Invalid, the drop guard will clear the memory.
            Err(_) => {
                return Err(DecodeErrorKind::InvalidUtf8);
            }
        }

        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl ProtoEncode for String {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        (self.len() as u64).encode_leb128(buf);
        buf.put_slice(self.as_bytes());
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        (self.len() as u64).encoded_leb128_len() + self.len()
    }
}

#[cfg(feature = "alloc")]
impl ProtoType for Vec<u8> {
    const WIRE_TYPE: WireType = WireType::Len;
}

#[cfg(feature = "alloc")]
impl ProtoDecode for Vec<u8> {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let len = crate::wire::decode_len(buf)?;
        if buf.remaining() < len {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }

        // Use a guard to safely handle uninitialized memory.
        struct Guard<'a> {
            buf: &'a mut Vec<u8>,
        }
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.buf.clear();
            }
        }

        let guard = Guard { buf: dst };

        // Clear in case this field appears multiple times.
        guard.buf.clear();
        guard.buf.reserve(len);
        // SAFETY: reserve guarantees capacity >= len, guard clears on panic.
        unsafe { guard.buf.set_len(len) };

        buf.copy_to_slice(&mut guard.buf[..len]);

        // Success, defuse the guard.
        core::mem::forget(guard);

        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl ProtoEncode for Vec<u8> {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        (self.len() as u64).encode_leb128(buf);
        buf.put_slice(self);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        (self.len() as u64).encoded_leb128_len() + self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn roundtrip<T: ProtoEncode + ProtoDecode + PartialEq + core::fmt::Debug + Default>(value: T) {
        let mut buf = Vec::new();
        value.encode(&mut buf);
        assert_eq!(buf.len(), value.encoded_len());
        let mut decoded = T::default();
        T::decode_into(&mut &buf[..], &mut decoded, 0).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn test_proto_bytes_roundtrip() {
        roundtrip(ProtoBytes::from(&[][..]));
        roundtrip(ProtoBytes::from(&[1, 2, 3][..]));
        roundtrip(ProtoBytes::from(&[0u8; 300][..]));
    }

    #[test]
    fn test_proto_string_roundtrip() {
        roundtrip(ProtoString::from(""));
        roundtrip(ProtoString::from("hello"));
        roundtrip(ProtoString::from("hello world! 🎉"));
    }

    #[test]
    fn test_proto_string_deref() {
        let s = ProtoString::from("hello");
        assert_eq!(&*s, "hello");
        assert_eq!(s.len(), 5);
        assert!(s.starts_with("hel"));
    }

    #[test]
    fn test_proto_string_invalid_utf8() {
        // Length prefix = 3, then invalid UTF-8 bytes
        let buf = &[3u8, 0xff, 0xfe, 0xfd][..];
        let mut decoded = ProtoString::default();
        let result = ProtoString::decode_into(&mut &buf[..], &mut decoded, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_string_roundtrip() {
        roundtrip(String::new());
        roundtrip(String::from("hello"));
        roundtrip(String::from("hello world! 🎉"));
        roundtrip("a".repeat(300));
    }

    #[test]
    fn test_string_invalid_utf8() {
        // Length prefix = 3, then invalid UTF-8 bytes
        let buf = &[3u8, 0xff, 0xfe, 0xfd][..];
        let mut decoded = String::default();
        let result = String::decode_into(&mut &buf[..], &mut decoded, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_vec_u8_roundtrip() {
        roundtrip(Vec::<u8>::new());
        roundtrip(vec![1u8, 2, 3]);
        roundtrip(vec![0u8; 300]);
        roundtrip((0u8..=255).collect::<Vec<_>>());
    }

    #[test]
    fn test_string_and_proto_string_compatible() {
        // Encode with String, decode with ProtoString
        let original = String::from("hello");
        let mut buf = Vec::new();
        original.encode(&mut buf);

        let mut decoded = ProtoString::default();
        ProtoString::decode_into(&mut &buf[..], &mut decoded, 0).unwrap();
        assert_eq!(decoded.as_str(), "hello");

        // Encode with ProtoString, decode with String
        let original = ProtoString::from("world");
        let mut buf = Vec::new();
        original.encode(&mut buf);

        let mut decoded = String::default();
        String::decode_into(&mut &buf[..], &mut decoded, 0).unwrap();
        assert_eq!(decoded, "world");
    }

    #[test]
    fn test_vec_u8_and_proto_bytes_compatible() {
        // Encode with Vec<u8>, decode with ProtoBytes
        let original = vec![1u8, 2, 3, 4, 5];
        let mut buf = Vec::new();
        original.encode(&mut buf);

        let mut decoded = ProtoBytes::default();
        ProtoBytes::decode_into(&mut &buf[..], &mut decoded, 0).unwrap();
        assert_eq!(&*decoded, &[1, 2, 3, 4, 5]);

        // Encode with ProtoBytes, decode with Vec<u8>
        let original = ProtoBytes::from(&[6u8, 7, 8, 9, 10][..]);
        let mut buf = Vec::new();
        original.encode(&mut buf);

        let mut decoded = Vec::<u8>::default();
        Vec::<u8>::decode_into(&mut &buf[..], &mut decoded, 0).unwrap();
        assert_eq!(decoded, vec![6, 7, 8, 9, 10]);
    }
}
