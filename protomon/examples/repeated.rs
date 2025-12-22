//! Test the derive macro with repeated nested messages.
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
use protomon::codec::{LazyMessage, ProtoMessage, ProtoString, Repeated};
use protomon::ProtoMessage as ProtoMessageDerive;

#[derive(Debug, Clone, PartialEq, ProtoMessageDerive)]
pub struct PhoneNumber {
    #[proto(tag = 1)]
    pub number: ProtoString,
    #[proto(tag = 2)]
    pub phone_type: i32,
}

/// Minimal allocation, create a `Vec` but lazily decode the inner type.
#[derive(Debug, ProtoMessageDerive)]
pub struct PersonMinimal {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(tag = 2)]
    pub id: i32,
    #[proto(tag = 3, repeated)]
    pub phones: Vec<LazyMessage<PhoneNumber>>,
}

/// Zero allocation! Repeated lazily decodes the inner type.
#[derive(Debug, ProtoMessageDerive)]
pub struct PersonZero {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(tag = 2)]
    pub id: i32,
    #[proto(tag = 3, repeated)]
    pub phones: Repeated<PhoneNumber>,
}

fn main() {
    // Create phones and wrap them in LazyMessage
    let phone1 = PhoneNumber {
        number: ProtoString::from("555-1234"),
        phone_type: 1,
    };
    let phone2 = PhoneNumber {
        number: ProtoString::from("555-5678"),
        phone_type: 2,
    };

    let person = PersonMinimal {
        name: ProtoString::from("Alice"),
        id: 123,
        phones: vec![
            LazyMessage::from_value(&phone1),
            LazyMessage::from_value(&phone2),
        ],
    };

    // Encode.
    let mut buf = Vec::new();
    person.encode_message(&mut buf);
    println!("Encoded {} bytes: {:02x?}", buf.len(), buf);

    // Decode back into PersonMinimal.
    let decoded = PersonMinimal::decode_message(Bytes::from(buf.clone())).unwrap();
    println!("{decoded:#?}");
    for (i, lazy_phone) in decoded.phones.iter().enumerate() {
        let phone = lazy_phone.decode().unwrap();
        println!("{i} {phone:?}");
    }

    // Decode into PersonZero.
    let lazy = PersonZero::decode_message(Bytes::from(buf)).unwrap();
    println!("\n{lazy:?}");
    for (i, result) in lazy.phones.iter().enumerate() {
        let phone = result.unwrap();
        println!("{i} {phone:?}");
    }
}
