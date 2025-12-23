//! Test the derive macro with a simple message.

use bytes::Bytes;
use protomon::codec::{ProtoMessage, ProtoString};
use protomon::ProtoMessage as ProtoMessageDerive;

#[derive(Debug, Default, PartialEq, ProtoMessageDerive)]
pub struct PhoneNumber {
    #[proto(tag = 1)]
    pub number: ProtoString,
    #[proto(tag = 2)]
    pub phone_type: i32,
}

fn main() {
    let phone = PhoneNumber {
        number: ProtoString::from("555-1234"),
        phone_type: 1,
    };

    // Encode.
    let mut buf = Vec::new();
    phone.encode_message(&mut buf);
    println!("Encoded {} bytes: {:02x?}", buf.len(), buf);

    // Decode.
    let decoded = PhoneNumber::decode_message(Bytes::from(buf)).unwrap();
    println!("Decoded: {decoded:?}");

    assert_eq!(phone, decoded);
    println!("Roundtrip successful!");
}
