//! Test the derive macro with a simple message.

use bytes::Bytes;
use protomon::codec::{ProtoEncode, ProtoMessage, ProtoString, ProtoType};
use protomon::wire::encode_key;
use protomon::ProtoMessage as ProtoMessageDerive;

#[derive(Debug, Default, ProtoMessageDerive)]
pub struct PhoneNumber {
    #[proto(tag = 1)]
    pub number: ProtoString,
    #[proto(tag = 2)]
    pub phone_type: i32,
}

fn main() {
    // Build a test message manually
    let mut buf = Vec::new();

    // Field 1: number = "555-1234"
    encode_key(ProtoString::WIRE_TYPE, 1, &mut buf);
    ProtoString::from("555-1234").encode(&mut buf);

    // Field 2: phone_type = 1
    encode_key(i32::WIRE_TYPE, 2, &mut buf);
    1i32.encode(&mut buf);

    println!("Encoded {} bytes: {:02x?}", buf.len(), buf);

    // Decode using the derived impl
    let phone = PhoneNumber::decode_message(Bytes::from(buf)).unwrap();

    println!("Decoded PhoneNumber:");
    println!("  number: {:?}", phone.number);
    println!("  phone_type: {}", phone.phone_type);

    // Test encoding
    let mut encoded = Vec::new();
    phone.encode_message(&mut encoded);
    println!("Re-encoded {} bytes: {:02x?}", encoded.len(), encoded);
}
