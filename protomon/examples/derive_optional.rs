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
use protomon::codec::{ProtoEncode, ProtoMessage, ProtoString};
use protomon::wire::{encode_key, WireType};
use protomon::ProtoMessage as ProtoMessageDerive;

#[derive(Debug, ProtoMessageDerive)]
pub struct User {
    #[proto(tag = 1)]
    pub name: ProtoString,
    #[proto(tag = 2, optional)]
    pub email: Option<ProtoString>,
    #[proto(tag = 3, optional)]
    pub age: Option<i32>,
}

fn main() {
    // Build a test message with all fields present
    let mut buf = Vec::new();

    // Field 1: name = "Alice"
    encode_key(WireType::Len, 1, &mut buf);
    ProtoString::from("Alice").encode(&mut buf);

    // Field 2: email = "alice@example.com"
    encode_key(WireType::Len, 2, &mut buf);
    ProtoString::from("alice@example.com").encode(&mut buf);

    // Field 3: age = 30
    encode_key(WireType::Varint, 3, &mut buf);
    30i32.encode(&mut buf);

    println!("=== Message with all fields ===");
    println!("Encoded {} bytes: {:02x?}", buf.len(), buf);

    let user = User::decode_message(Bytes::from(buf)).unwrap();
    println!("Decoded User:");
    println!("  name: {:?}", user.name);
    println!("  email: {:?}", user.email);
    println!("  age: {:?}", user.age);

    // Re-encode
    let mut re_encoded = Vec::new();
    user.encode_message(&mut re_encoded);
    println!("Re-encoded {} bytes: {:02x?}", re_encoded.len(), re_encoded);
    println!();

    // Build a test message with optional fields missing
    let mut buf2 = Vec::new();

    // Field 1: name = "Bob"
    encode_key(WireType::Len, 1, &mut buf2);
    ProtoString::from("Bob").encode(&mut buf2);

    // No email or age

    println!("=== Message with optional fields missing ===");
    println!("Encoded {} bytes: {:02x?}", buf2.len(), buf2);

    let user2 = User::decode_message(Bytes::from(buf2)).unwrap();
    println!("Decoded User:");
    println!("  name: {:?}", user2.name);
    println!("  email: {:?}", user2.email);
    println!("  age: {:?}", user2.age);

    // Re-encode - should be same size since None fields aren't encoded
    let mut re_encoded2 = Vec::new();
    user2.encode_message(&mut re_encoded2);
    println!("Re-encoded {} bytes: {:02x?}", re_encoded2.len(), re_encoded2);
    println!("encoded_message_len: {}", user2.encoded_message_len());
}
