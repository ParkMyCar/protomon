//! Encoding and decoding traits for protobuf wire format.

mod delimited;
mod message;
mod optional;
mod repeated;
mod scalar;

use crate::error::DecodeErrorKind;
use crate::wire::WireType;

pub trait ProtoType: Sized {
    /// The wire type used to decode this type.
    const WIRE_TYPE: WireType;
}

/// A type that can be decoded from protobuf wire format.
///
/// The `decode_into` method follows protobuf merging semantics:
/// - Scalars: last value wins (overwrite)
/// - Embedded messages: recursive merge
/// - Repeated fields: append
pub trait ProtoDecode: ProtoType {
    /// Initialize an empty instance of `Self` for decoding.
    ///
    /// # Parameters
    /// - `msg_buf`: Buffer for the entire message. Most implementations ignore this,
    ///   but types like `Repeated` use it for lazy iteration.
    /// - `tag`: The field tag number. Most implementations ignore this, but types
    ///   like `Repeated` use it for scan mode iteration.
    fn init<B: bytes::Buf>(msg_buf: B, tag: u32) -> Self;

    /// Decode from buffer into dst, following protobuf merging semantics.
    ///
    /// The `offset` parameter is the byte offset within the original message buffer
    /// where this value starts. This is used by types like `Repeated` that need to
    /// track value positions for lazy iteration.
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        offset: usize,
    ) -> Result<(), DecodeErrorKind>;
}

/// A type that can be encoded to protobuf wire format.
///
/// Types that implement `ProtoEncode` must also implement `ProtoDecode`.
pub trait ProtoEncode: ProtoType {
    /// Encode this value to the buffer.
    fn encode<B: bytes::BufMut>(&self, buf: &mut B);

    /// Returns the encoded length of this value (not including field key).
    fn encoded_len(&self) -> usize;
}

// Re-export scalar types
pub use scalar::{Fixed32, Fixed64, Sfixed32, Sfixed64, Sint32, Sint64};

// Re-export length-delimited types
pub use delimited::{ProtoBytes, ProtoString};

// Re-export repeated field types
pub use repeated::{CloneableIterator, PackedIter, Repeated, RepeatedDecodeIter, RepeatedIter};

// Re-export message types and helpers
pub use message::{
    decode_message_field, encode_message_field, encoded_message_field_len, skip_len_field,
    LazyMessage, ProtoMessage,
};
