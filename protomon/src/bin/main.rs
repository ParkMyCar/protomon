//! Example protobuf message decoding using protomon.
//!
//! This demonstrates what generated code would look like for:
//!
//! ```proto
//! message PhoneNumber {
//!     string number = 1;
//!     int32 type = 2;
//! }
//!
//! message Person {
//!     string name = 1;
//!     int32 id = 2;
//!     string email = 3;
//!     repeated PhoneNumber phones = 4;
//! }
//! ```

use bytes::{Buf, Bytes};
use protomon::codec::{
    LazyMessage, ProtoDecode, ProtoEncode, ProtoMessage, ProtoString, ProtoType, Repeated,
    RepeatedIter, encode_message_field,
};
use protomon::error::DecodeErrorKind;
use protomon::wire::{WireType, decode_key, encode_key, skip_field};

#[derive(Debug, Clone, Default)]
pub struct PhoneNumber {
    number: ProtoString,
    phone_type: i32,
}

impl PhoneNumber {
    pub fn number(&self) -> &str {
        &self.number
    }

    pub fn phone_type(&self) -> i32 {
        self.phone_type
    }
}

impl ProtoMessage for PhoneNumber {
    fn decode_message(buf: Bytes) -> Result<Self, DecodeErrorKind> {
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
        Ok(PhoneNumber { number, phone_type })
    }

    fn encode_message<B: bytes::BufMut>(&self, buf: &mut B) {
        if !self.number.is_empty() {
            encode_key(ProtoString::WIRE_TYPE, 1, buf);
            self.number.encode(buf);
        }
        if self.phone_type != 0 {
            encode_key(i32::WIRE_TYPE, 2, buf);
            self.phone_type.encode(buf);
        }
    }

    fn encoded_message_len(&self) -> usize {
        let mut len = 0;
        if !self.number.is_empty() {
            len += 1 + self.number.encoded_len();
        }
        if self.phone_type != 0 {
            len += 1 + self.phone_type.encoded_len();
        }
        len
    }
}

#[derive(Clone)]
pub struct Person {
    name: ProtoString,
    id: i32,
    email: ProtoString,
    phones: Repeated<LazyMessage<PhoneNumber>>,
}

impl Person {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn phones(&self) -> RepeatedIter<'_, LazyMessage<PhoneNumber>> {
        self.phones.iter()
    }

    pub fn phones_len(&self) -> usize {
        self.phones.len()
    }
}

impl ProtoMessage for Person {
    fn decode_message(buf: Bytes) -> Result<Self, DecodeErrorKind> {
        let mut slice = &buf[..];
        // Initialize all fields using ProtoDecode::init
        let mut name = ProtoString::init(buf.clone(), 1);
        let mut id = i32::init(buf.clone(), 2);
        let mut email = ProtoString::init(buf.clone(), 3);
        let mut phones: Repeated<LazyMessage<PhoneNumber>> =
            <Repeated<LazyMessage<PhoneNumber>> as ProtoDecode>::init(buf.clone(), 4);

        while slice.has_remaining() {
            let (wire_type, tag) = decode_key(&mut slice)?;
            // Value offset is after key decode
            let value_offset = buf.len() - slice.len();
            match tag {
                1 => ProtoString::decode_into(&mut slice, &mut name, value_offset)?,
                2 => i32::decode_into(&mut slice, &mut id, value_offset)?,
                3 => ProtoString::decode_into(&mut slice, &mut email, value_offset)?,
                // All fields use decode_into uniformly
                4 => <Repeated<LazyMessage<PhoneNumber>> as ProtoDecode>::decode_into(
                    &mut slice,
                    &mut phones,
                    value_offset,
                )?,
                _ => skip_field(wire_type, &mut slice)?,
            }
        }

        Ok(Person {
            name,
            id,
            email,
            phones,
        })
    }

    fn encode_message<B: bytes::BufMut>(&self, buf: &mut B) {
        if !self.name.is_empty() {
            encode_key(ProtoString::WIRE_TYPE, 1, buf);
            self.name.encode(buf);
        }
        if self.id != 0 {
            encode_key(i32::WIRE_TYPE, 2, buf);
            self.id.encode(buf);
        }
        if !self.email.is_empty() {
            encode_key(ProtoString::WIRE_TYPE, 3, buf);
            self.email.encode(buf);
        }
        // Note: encoding repeated fields requires storing them, which we don't do
        // in this zero-copy design. For encoding, you'd need a separate builder pattern.
    }

    fn encoded_message_len(&self) -> usize {
        let mut len = 0;
        if !self.name.is_empty() {
            len += 1 + self.name.encoded_len();
        }
        if self.id != 0 {
            len += 1 + self.id.encoded_len();
        }
        if !self.email.is_empty() {
            len += 1 + self.email.encoded_len();
        }
        len
    }
}

fn main() {
    // Build a test message using raw encoding
    let mut buf = Vec::new();

    // Field 1: name = "Alice"
    encode_key(ProtoString::WIRE_TYPE, 1, &mut buf);
    ProtoString::from("Alice").encode(&mut buf);

    // Field 2: id = 123
    encode_key(i32::WIRE_TYPE, 2, &mut buf);
    123i32.encode(&mut buf);

    // Field 3: email = "alice@example.com"
    encode_key(ProtoString::WIRE_TYPE, 3, &mut buf);
    ProtoString::from("alice@example.com").encode(&mut buf);

    // Field 4: phones[0] = PhoneNumber { number: "555-1234", type: 1 }
    encode_key(WireType::Len, 4, &mut buf);
    let phone1 = PhoneNumber {
        number: ProtoString::from("555-1234"),
        phone_type: 1,
    };
    encode_message_field(&phone1, &mut buf);

    // Field 4: phones[1] = PhoneNumber { number: "555-5678", type: 2 }
    encode_key(WireType::Len, 4, &mut buf);
    let phone2 = PhoneNumber {
        number: ProtoString::from("555-5678"),
        phone_type: 2,
    };
    encode_message_field(&phone2, &mut buf);

    println!("Encoded {} bytes", buf.len());
    println!("Raw bytes: {:02x?}", buf);
    println!();

    // Decode the message
    let person = Person::decode_message(Bytes::from(buf)).unwrap();

    println!("Decoded Person:");
    println!("  name: {}", person.name());
    println!("  id: {}", person.id());
    println!("  email: {}", person.email());
    println!("  phones_len: {}", person.phones_len());

    // Iterate over phones (lazy decoding)
    for (i, phone_result) in person.phones().enumerate() {
        let lazy_phone = phone_result.unwrap();
        let phone = lazy_phone.decode().unwrap();
        println!(
            "  phone[{}]: {} (type={})",
            i,
            phone.number(),
            phone.phone_type()
        );
    }
}
