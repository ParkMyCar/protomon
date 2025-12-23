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
use protomon::codec::{ProtoMessage, ProtoString};
use protomon::ProtoMessage as ProtoMessageDerive;

#[derive(Debug, Default, PartialEq, ProtoMessageDerive)]
pub struct Scores {
    #[proto(tag = 1)]
    pub player_name: ProtoString,
    #[proto(tag = 2, repeated)]
    pub scores: Vec<i32>,
    #[proto(tag = 3, repeated)]
    pub tags: Vec<ProtoString>,
}

fn main() {
    let scores = Scores {
        player_name: ProtoString::from("Alice"),
        scores: vec![100, 95, 87],
        tags: vec![ProtoString::from("pro"), ProtoString::from("verified")],
    };

    let mut buf = Vec::new();
    scores.encode_message(&mut buf);
    println!("=== Vec<T> repeated fields ===");
    println!("Encoded {} bytes: {:02x?}", buf.len(), buf);

    let decoded = Scores::decode_message(Bytes::from(buf)).unwrap();
    println!(
        "Decoded: player={:?}, scores={:?}, tags={:?}",
        decoded.player_name,
        decoded.scores,
        decoded.tags.iter().map(|s| s.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(scores, decoded);

    // Empty repeated fields
    let empty = Scores {
        player_name: ProtoString::from("Bob"),
        scores: vec![],
        tags: vec![],
    };

    let mut buf2 = Vec::new();
    empty.encode_message(&mut buf2);
    println!("\n=== Empty repeated fields ===");
    println!("Encoded {} bytes: {:02x?}", buf2.len(), buf2);

    let decoded2 = Scores::decode_message(Bytes::from(buf2)).unwrap();
    println!(
        "Decoded: player={:?}, scores={:?}, tags={:?}",
        decoded2.player_name, decoded2.scores, decoded2.tags
    );
    assert_eq!(empty, decoded2);

    println!("\nRoundtrip successful!");
}
