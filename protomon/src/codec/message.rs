//! Message-level types and helpers.

use crate::error::DecodeErrorKind;
use crate::leb128::LebCodec;
use crate::wire::WireType;
use super::{ProtoDecode, ProtoEncode, ProtoType};

/// Trait for protobuf message types.
///
/// This trait is implemented by generated message structs. It handles decoding
/// the message body (without length prefix). The `ProtoDecode` impl for messages
/// handles reading the length prefix first.
pub trait ProtoMessage: Sized {
    /// Decode a message from the given bytes buffer.
    ///
    /// Takes ownership of `Bytes` to allow zero-copy storage for repeated field iteration.
    /// Consumes all bytes in the buffer.
    fn decode_message(buf: bytes::Bytes) -> Result<Self, DecodeErrorKind>;

    /// Encode the message body (without length prefix).
    fn encode_message<B: bytes::BufMut>(&self, buf: &mut B);

    /// Returns the encoded length of the message body (without length prefix).
    fn encoded_message_len(&self) -> usize;
}

/// Helper to decode a message as a length-delimited field.
///
/// This is the pattern generated code uses for nested message fields.
#[inline]
pub fn decode_message_field<T: ProtoMessage, B: bytes::Buf>(
    buf: &mut B,
) -> Result<T, DecodeErrorKind> {
    let len = crate::wire::decode_len(buf)?;
    if buf.remaining() < len {
        return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
    }
    let message_bytes = buf.copy_to_bytes(len);
    T::decode_message(message_bytes)
}

/// Helper to encode a message as a length-delimited field.
///
/// Writes the length prefix followed by the message body.
#[inline]
pub fn encode_message_field<T: ProtoMessage, B: bytes::BufMut>(msg: &T, buf: &mut B) {
    let msg_len = msg.encoded_message_len();
    (msg_len as u64).encode_leb128(buf);
    msg.encode_message(buf);
}

/// Returns the encoded length of a message as a length-delimited field.
#[inline]
pub fn encoded_message_field_len<T: ProtoMessage>(msg: &T) -> usize {
    let msg_len = msg.encoded_message_len();
    (msg_len as u64).encoded_leb128_len() + msg_len
}

/// Lazy wrapper for nested message fields.
///
/// Stores the raw message bytes (without length prefix) and decodes on demand.
/// This allows skipping over nested messages during the initial decode pass.
#[derive(Clone)]
pub struct LazyMessage<T> {
    buf: bytes::Bytes,
    _marker: core::marker::PhantomData<T>,
}

impl<T> LazyMessage<T> {
    /// Create a new lazy message wrapper from raw message bytes.
    pub fn new(buf: bytes::Bytes) -> Self {
        Self {
            buf,
            _marker: core::marker::PhantomData,
        }
    }

    /// Returns the raw message bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Returns the underlying Bytes.
    pub fn into_bytes(self) -> bytes::Bytes {
        self.buf
    }
}

impl<T: ProtoMessage> LazyMessage<T> {
    /// Decode the message. Can be called multiple times.
    pub fn decode(&self) -> Result<T, DecodeErrorKind> {
        T::decode_message(self.buf.clone())
    }
}

impl<T: ProtoMessage> ProtoType for LazyMessage<T> {
    const WIRE_TYPE: WireType = WireType::Len;
}

impl<T: ProtoMessage> ProtoDecode for LazyMessage<T> {
    #[inline]
    fn init<B: bytes::Buf>(_msg_buf: B, _tag: u32) -> Self {
        LazyMessage::new(bytes::Bytes::new())
    }

    /// Decodes the length prefix and stores the message bytes for lazy decoding.
    #[inline]
    fn decode_into<B: bytes::Buf>(buf: &mut B, dst: &mut Self, _offset: usize) -> Result<(), DecodeErrorKind> {
        let len = crate::wire::decode_len(buf)?;
        if buf.remaining() < len {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        *dst = LazyMessage::new(buf.copy_to_bytes(len));
        Ok(())
    }
}

impl<T: ProtoMessage> ProtoEncode for LazyMessage<T> {
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        (self.buf.len() as u64).encode_leb128(buf);
        buf.put_slice(&self.buf);
    }

    #[inline]
    fn encoded_len(&self) -> usize {
        (self.buf.len() as u64).encoded_leb128_len() + self.buf.len()
    }
}

impl<T> Default for LazyMessage<T> {
    fn default() -> Self {
        Self::new(bytes::Bytes::new())
    }
}

impl<T> std::fmt::Debug for LazyMessage<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LazyMessage")
            .field("len", &self.buf.len())
            .finish()
    }
}

/// Skip over a length-delimited field and return its bytes (without the length prefix).
///
/// Use this during message decoding to lazily skip nested messages.
#[inline]
pub fn skip_len_field<B: bytes::Buf>(buf: &mut B) -> Result<bytes::Bytes, DecodeErrorKind> {
    let len = crate::wire::decode_len(buf)?;
    if buf.remaining() < len {
        return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
    }
    Ok(buf.copy_to_bytes(len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{ProtoDecode, ProtoEncode, ProtoString};
    use bytes::Buf;
    use crate::wire::{decode_key, encode_key, skip_field, WireType};

    /// Inner message: `message PhoneNumber { string number = 1; int32 type = 2; }`
    #[derive(Debug, Clone, PartialEq, Default)]
    struct PhoneNumber {
        buf: bytes::Bytes,
        number: ProtoString,
        phone_type: i32,
    }

    impl ProtoMessage for PhoneNumber {
        fn decode_message(buf: bytes::Bytes) -> Result<Self, DecodeErrorKind> {
            let mut slice = &buf[..];
            let mut number = ProtoString::default();
            let mut phone_type = 0i32;

            while slice.has_remaining() {
                let (wire_type, tag) = decode_key(&mut slice)?;
                let value_offset = buf.len() - slice.len();
                match tag {
                    1 => ProtoString::decode_into(&mut slice, &mut number, value_offset)?,
                    2 => i32::decode_into(&mut slice, &mut phone_type, value_offset)?,
                    _ => skip_field(wire_type, &mut slice)?,
                }
            }
            Ok(PhoneNumber { buf, number, phone_type })
        }

        fn encode_message<B: bytes::BufMut>(&self, buf: &mut B) {
            if !self.number.is_empty() {
                encode_key(WireType::Len, 1, buf);
                self.number.encode(buf);
            }
            if self.phone_type != 0 {
                encode_key(WireType::Varint, 2, buf);
                self.phone_type.encode(buf);
            }
        }

        fn encoded_message_len(&self) -> usize {
            let mut len = 0;
            if !self.number.is_empty() {
                len += 1 + self.number.encoded_len(); // key + value
            }
            if self.phone_type != 0 {
                len += 1 + self.phone_type.encoded_len();
            }
            len
        }
    }

    /// Outer message: `message Person { string name = 1; PhoneNumber phone = 2; }`
    #[derive(Debug, Clone, PartialEq, Default)]
    struct Person {
        buf: bytes::Bytes,
        name: ProtoString,
        phone: Option<PhoneNumber>,
    }

    impl ProtoMessage for Person {
        fn decode_message(buf: bytes::Bytes) -> Result<Self, DecodeErrorKind> {
            let mut slice = &buf[..];
            let mut name = ProtoString::default();
            let mut phone = None;

            while slice.has_remaining() {
                let (wire_type, tag) = decode_key(&mut slice)?;
                let value_offset = buf.len() - slice.len();
                match tag {
                    1 => ProtoString::decode_into(&mut slice, &mut name, value_offset)?,
                    2 => phone = Some(decode_message_field(&mut slice)?),
                    _ => skip_field(wire_type, &mut slice)?,
                }
            }
            Ok(Person { buf, name, phone })
        }

        fn encode_message<B: bytes::BufMut>(&self, buf: &mut B) {
            if !self.name.is_empty() {
                encode_key(WireType::Len, 1, buf);
                self.name.encode(buf);
            }
            if let Some(ref phone) = self.phone {
                encode_key(WireType::Len, 2, buf);
                encode_message_field(phone, buf);
            }
        }

        fn encoded_message_len(&self) -> usize {
            let mut len = 0;
            if !self.name.is_empty() {
                len += 1 + self.name.encoded_len();
            }
            if let Some(ref phone) = self.phone {
                len += 1 + encoded_message_field_len(phone);
            }
            len
        }
    }

    #[test]
    fn test_nested_message_roundtrip() {
        let phone = PhoneNumber {
            buf: bytes::Bytes::new(),
            number: ProtoString::from("555-1234"),
            phone_type: 1,
        };
        let person = Person {
            buf: bytes::Bytes::new(),
            name: ProtoString::from("Alice"),
            phone: Some(phone),
        };

        // Encode
        let mut buf = Vec::new();
        person.encode_message(&mut buf);

        // Decode
        let decoded = Person::decode_message(bytes::Bytes::from(buf)).unwrap();

        assert_eq!(&*decoded.name, "Alice");
        let decoded_phone = decoded.phone.as_ref().unwrap();
        assert_eq!(&*decoded_phone.number, "555-1234");
        assert_eq!(decoded_phone.phone_type, 1);
    }

    #[test]
    fn test_nested_message_as_field() {
        // Test decoding a nested message when it appears as a field
        // (with length prefix, as it would in a parent message)
        let phone = PhoneNumber {
            buf: bytes::Bytes::new(),
            number: ProtoString::from("555-1234"),
            phone_type: 2,
        };

        // Encode as length-delimited field
        let mut buf = Vec::new();
        encode_message_field(&phone, &mut buf);

        // Decode
        let decoded: PhoneNumber = decode_message_field(&mut &buf[..]).unwrap();
        assert_eq!(&*decoded.number, "555-1234");
        assert_eq!(decoded.phone_type, 2);
    }

    /// Example using LazyMessage for deferred nested message decoding.
    #[derive(Debug, Clone, Default)]
    struct PersonLazy {
        name: ProtoString,
        phone: Option<LazyMessage<PhoneNumber>>,
    }

    impl PersonLazy {
        fn decode(buf: bytes::Bytes) -> Result<Self, DecodeErrorKind> {
            let mut slice = &buf[..];
            let mut name = ProtoString::default();
            let mut phone = None;

            while slice.has_remaining() {
                let (wire_type, tag) = decode_key(&mut slice)?;
                let value_offset = buf.len() - slice.len();
                match tag {
                    1 => ProtoString::decode_into(&mut slice, &mut name, value_offset)?,
                    // Skip over the nested message, store bytes for later
                    2 => phone = Some(LazyMessage::new(skip_len_field(&mut slice)?)),
                    _ => skip_field(wire_type, &mut slice)?,
                }
            }
            Ok(PersonLazy { name, phone })
        }

        fn phone(&self) -> Option<Result<PhoneNumber, DecodeErrorKind>> {
            self.phone.as_ref().map(|lazy| lazy.decode())
        }
    }

    #[test]
    fn test_lazy_nested_message() {
        let phone = PhoneNumber {
            buf: bytes::Bytes::new(),
            number: ProtoString::from("555-1234"),
            phone_type: 1,
        };
        let person = Person {
            buf: bytes::Bytes::new(),
            name: ProtoString::from("Bob"),
            phone: Some(phone),
        };

        // Encode the eager person
        let mut buf = Vec::new();
        person.encode_message(&mut buf);

        // Decode as lazy - PhoneNumber is NOT parsed yet
        let lazy_person = PersonLazy::decode(bytes::Bytes::from(buf)).unwrap();
        assert_eq!(&*lazy_person.name, "Bob");

        // Now decode the phone on demand
        let decoded_phone = lazy_person.phone().unwrap().unwrap();
        assert_eq!(&*decoded_phone.number, "555-1234");
        assert_eq!(decoded_phone.phone_type, 1);

        // Can decode again (LazyMessage is reusable)
        let decoded_phone2 = lazy_person.phone().unwrap().unwrap();
        assert_eq!(&*decoded_phone2.number, "555-1234");
    }
}
