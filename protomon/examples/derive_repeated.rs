//! Test the derive macro with repeated fields.
//!
//! Equivalent to:
//! ```proto
//! message PhoneNumber {
//!     string number = 1;
//!     int32 type = 2;
//! }
//!
//! message Person {
//!     string name = 1;
//!     int32 id = 2;
//!     repeated PhoneNumber phones = 3;
//! }
//! ```

use bytes::Bytes;
use protomon::codec::{
    encode_message_field, LazyMessage, ProtoEncode, ProtoMessage, ProtoString, Repeated,
};
use protomon::wire::{encode_key, WireType};
use protomon::ProtoMessage as ProtoMessageDerive;

#[derive(Debug, Default, Clone, ProtoMessageDerive)]
pub struct PhoneNumber {
    #[proto(tag = 1)]
    pub number: ProtoString,
    #[proto(tag = 2)]
    pub phone_type: i32,
}

#[derive(Clone, ProtoMessageDerive)]
pub struct Person {
    #[proto(tag = 1)]
    name: ProtoString,
    #[proto(tag = 2)]
    id: i32,
    #[proto(tag = 3, repeated)]
    phones: Repeated<LazyMessage<PhoneNumber>>,
}

fn main() {
    // Build a test message manually
    let mut buf = Vec::new();

    // Field 1: name = "Alice"
    encode_key(WireType::Len, 1, &mut buf);
    ProtoString::from("Alice").encode(&mut buf);

    // Field 2: id = 123
    encode_key(WireType::Varint, 2, &mut buf);
    123i32.encode(&mut buf);

    // Field 3: phones[0]
    encode_key(WireType::Len, 3, &mut buf);
    let phone1 = PhoneNumber {
        number: ProtoString::from("555-1234"),
        phone_type: 1,
    };
    encode_message_field(&phone1, &mut buf);

    // Field 3: phones[1]
    encode_key(WireType::Len, 3, &mut buf);
    let phone2 = PhoneNumber {
        number: ProtoString::from("555-5678"),
        phone_type: 2,
    };
    encode_message_field(&phone2, &mut buf);

    println!("Encoded {} bytes", buf.len());
    println!("Raw bytes: {:02x?}", buf);
    println!();

    // Decode using the derived impl
    let person = Person::decode_message(Bytes::from(buf)).unwrap();

    println!("Decoded Person:");
    println!("  name: {:?}", person.name);
    println!("  id: {}", person.id);
    println!("  phones.len(): {}", person.phones.len());

    // Iterate over phones (lazy decoding)
    for (i, phone_result) in person.phones.iter().enumerate() {
        let lazy_phone = phone_result.unwrap();
        let phone = lazy_phone.decode().unwrap();
        println!(
            "  phone[{}]: {:?} (type={})",
            i, phone.number, phone.phone_type
        );
    }
}
