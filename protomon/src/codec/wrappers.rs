//! Wrapper type support for protobuf (e.g. Option, Box).

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

use super::{ProtoDecode, ProtoEncode, ProtoType};
use crate::error::DecodeError;
use crate::wire::WireType;

impl<T: ProtoType> ProtoType for Option<T> {
    const WIRE_TYPE: WireType = T::WIRE_TYPE;
}

impl<T: ProtoDecode> ProtoDecode for Option<T> {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        offset: usize,
    ) -> Result<(), DecodeError> {
        let mut value = T::default();
        T::decode_into(buf, &mut value, offset)?;
        *dst = Some(value);
        Ok(())
    }
}

impl<T: ProtoEncode> ProtoEncode for Option<T> {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        if let Some(value) = self {
            value.encode(buf);
        }
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        match self {
            Some(value) => value.encoded_len(),
            None => 0,
        }
    }
}

// Box<T> implementations - allows heap allocation for recursive or large types.

#[cfg(feature = "alloc")]
impl<T: ProtoType> ProtoType for Box<T> {
    const WIRE_TYPE: WireType = T::WIRE_TYPE;
}

#[cfg(feature = "alloc")]
impl<T: ProtoDecode> ProtoDecode for Box<T> {
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        offset: usize,
    ) -> Result<(), DecodeError> {
        T::decode_into(buf, dst.as_mut(), offset)
    }
}

#[cfg(feature = "alloc")]
impl<T: ProtoEncode> ProtoEncode for Box<T> {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        self.as_ref().encode(buf);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        self.as_ref().encoded_len()
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::*;
    use crate::codec::ProtoString;

    #[test]
    fn test_option_none_default() {
        let opt: Option<i32> = Option::<i32>::default();
        assert!(opt.is_none());
    }

    #[test]
    fn test_option_decode_into() {
        // Encode a varint
        let buf = [0x96, 0x01]; // 150 in varint
        let mut opt: Option<i32> = None;
        <Option<i32> as ProtoDecode>::decode_into(&mut &buf[..], &mut opt, 0).unwrap();
        assert_eq!(opt, Some(150));
    }

    #[test]
    fn test_option_encode_some() {
        let opt: Option<i32> = Some(150);
        let mut buf = Vec::new();
        opt.encode(&mut buf);
        assert_eq!(buf, vec![0x96, 0x01]);
        assert_eq!(opt.encoded_len(), 2);
    }

    #[test]
    fn test_option_encode_none() {
        let opt: Option<i32> = None;
        let mut buf = Vec::new();
        opt.encode(&mut buf);
        assert!(buf.is_empty());
        assert_eq!(opt.encoded_len(), 0);
    }

    #[test]
    fn test_option_string() {
        let opt: Option<ProtoString> = Option::<ProtoString>::default();
        assert!(opt.is_none());

        // Decode a string
        let buf = [5, b'h', b'e', b'l', b'l', b'o']; // length-prefixed "hello"
        let mut opt: Option<ProtoString> = None;
        <Option<ProtoString> as ProtoDecode>::decode_into(&mut &buf[..], &mut opt, 0).unwrap();
        assert_eq!(opt.as_ref().map(|s| s.as_str()), Some("hello"));
    }

    #[test]
    fn test_box_decode_into() {
        // Encode a varint
        let buf = [0x96, 0x01]; // 150 in varint
        let mut boxed: Box<i32> = Box::new(0);
        <Box<i32> as ProtoDecode>::decode_into(&mut &buf[..], &mut boxed, 0).unwrap();
        assert_eq!(*boxed, 150);
    }

    #[test]
    fn test_box_encode() {
        let boxed: Box<i32> = Box::new(150);
        let mut buf = Vec::new();
        boxed.encode(&mut buf);
        assert_eq!(buf, vec![0x96, 0x01]);
        assert_eq!(boxed.encoded_len(), 2);
    }

    #[test]
    fn test_box_string_roundtrip() {
        let original: Box<ProtoString> = Box::new(ProtoString::from("hello"));

        // Encode
        let mut buf = Vec::new();
        original.encode(&mut buf);
        assert_eq!(original.encoded_len(), buf.len());

        // Decode
        let mut decoded: Box<ProtoString> = Box::default();
        <Box<ProtoString> as ProtoDecode>::decode_into(&mut &buf[..], &mut decoded, 0).unwrap();
        assert_eq!(decoded.as_str(), "hello");
    }
}
