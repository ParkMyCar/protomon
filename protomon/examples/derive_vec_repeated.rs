//! Test the derive macro with Vec<T> repeated fields.
//!
//! Equivalent to:
//! ```proto
//! message Scores {
//!     string player_name = 1;
//!     repeated int32 scores = 2;
//!     repeated string tags = 3;
//! }
//! ```

use bytes::Bytes;
use protomon::codec::{ProtoEncode, ProtoMessage, ProtoString};
use protomon::wire::{encode_key, WireType};
use protomon::ProtoMessage as ProtoMessageDerive;

#[derive(Debug, ProtoMessageDerive)]
pub struct Scores {
    #[proto(tag = 1)]
    pub player_name: ProtoString,
    #[proto(tag = 2, repeated)]
    pub scores: Vec<i32>,
    #[proto(tag = 3, repeated)]
    pub tags: Vec<ProtoString>,
}

fn main() {
    // Build a test message manually
    let mut buf = Vec::new();

    // Field 1: player_name = "Alice"
    encode_key(WireType::Len, 1, &mut buf);
    ProtoString::from("Alice").encode(&mut buf);

    // Field 2: scores = [100, 95, 87]
    encode_key(WireType::Varint, 2, &mut buf);
    100i32.encode(&mut buf);
    encode_key(WireType::Varint, 2, &mut buf);
    95i32.encode(&mut buf);
    encode_key(WireType::Varint, 2, &mut buf);
    87i32.encode(&mut buf);

    // Field 3: tags = ["pro", "verified"]
    encode_key(WireType::Len, 3, &mut buf);
    ProtoString::from("pro").encode(&mut buf);
    encode_key(WireType::Len, 3, &mut buf);
    ProtoString::from("verified").encode(&mut buf);

    println!("=== Decoding Vec<T> repeated fields ===");
    println!("Encoded {} bytes: {:02x?}", buf.len(), buf);

    let scores = Scores::decode_message(Bytes::from(buf)).unwrap();
    println!("Decoded Scores:");
    println!("  player_name: {:?}", scores.player_name);
    println!("  scores: {:?}", scores.scores);
    println!(
        "  tags: {:?}",
        scores
            .tags
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
    );

    // Re-encode
    let mut re_encoded = Vec::new();
    scores.encode_message(&mut re_encoded);
    println!("Re-encoded {} bytes: {:02x?}", re_encoded.len(), re_encoded);
    println!("encoded_message_len: {}", scores.encoded_message_len());
    println!();

    // Test empty repeated fields
    println!("=== Empty repeated fields ===");
    let mut buf2 = Vec::new();
    encode_key(WireType::Len, 1, &mut buf2);
    ProtoString::from("Bob").encode(&mut buf2);
    // No scores or tags

    let scores2 = Scores::decode_message(Bytes::from(buf2)).unwrap();
    println!("Decoded Scores:");
    println!("  player_name: {:?}", scores2.player_name);
    println!("  scores: {:?}", scores2.scores);
    println!("  tags: {:?}", scores2.tags);

    let mut re_encoded2 = Vec::new();
    scores2.encode_message(&mut re_encoded2);
    println!("Re-encoded {} bytes: {:02x?}", re_encoded2.len(), re_encoded2);
    println!("encoded_message_len: {}", scores2.encoded_message_len());
}
