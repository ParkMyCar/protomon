//! Length-delimited protobuf types (bytes, string).

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
    fn init<B: bytes::Buf>(_msg_buf: B, _tag: u32) -> Self {
        ProtoBytes(bytes::Bytes::new())
    }

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
        unsafe { std::str::from_utf8_unchecked(&self.0) }
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
    fn init<B: bytes::Buf>(_msg_buf: B, _tag: u32) -> Self {
        ProtoString(bytes::Bytes::new())
    }

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
        if std::str::from_utf8(&data).is_err() {
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
}
