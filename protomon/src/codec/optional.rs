//! Optional field support for protobuf.

use super::{ProtoDecode, ProtoEncode, ProtoType};
use crate::error::DecodeErrorKind;
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
    ) -> Result<(), DecodeErrorKind> {
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

#[cfg(test)]
mod tests {
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
}
