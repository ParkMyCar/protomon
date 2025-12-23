//! Test the derive macro with optional fields.
//!
//! Equivalent to:
//! ```proto
//! message User {
//!     string name = 1;
//!     optional string email = 2;
//!     optional int32 age = 3;
//! }
//! ```

use bytes::Bytes;
use protomon::codec::{ProtoMessage, ProtoString};
use protomon::ProtoMessage as ProtoMessageDerive;

#[derive(Debug, Default, PartialEq, ProtoMessageDerive)]
pub struct User {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(tag = 2, optional)]
    pub email: Option<ProtoString>,
    #[proto(tag = 3, optional)]
    pub age: Option<i32>,
}

fn main() {
    // User with all fields.
    let user = User {
        name: ProtoString::from("Alice"),
        email: Some(ProtoString::from("alice@example.com")),
        age: Some(30),
    };

    let mut buf = Vec::new();
    user.encode_message(&mut buf);
    println!("Encoded {} bytes: {:02x?}", buf.len(), buf);

    let decoded = User::decode_message(Bytes::from(buf)).unwrap();
    println!("{decoded:?}");
    assert_eq!(user, decoded);

    // User with optional fields missing.
    let user2 = User {
        name: ProtoString::from("Bob"),
        email: None,
        age: None,
    };

    let mut buf2 = Vec::new();
    user2.encode_message(&mut buf2);
    println!("Encoded {} bytes: {:02x?}", buf2.len(), buf2);

    let decoded2 = User::decode_message(Bytes::from(buf2)).unwrap();
    println!("{decoded2:?}");
    assert_eq!(user2, decoded2);

    println!("\nRoundtrip successful!");
}
