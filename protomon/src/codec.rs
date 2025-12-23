//! Encoding and decoding traits for protobuf wire format.

mod default_check;
mod delimited;
#[cfg(feature = "alloc")]
mod map;
mod message;
mod oneof;
#[cfg(feature = "alloc")]
mod packed;
#[cfg(feature = "alloc")]
mod repeated;
mod scalar;
mod wrappers;

use crate::error::DecodeErrorKind;
use crate::wire::WireType;

pub trait ProtoType: Sized {
    /// The wire type used to decode this type.
    const WIRE_TYPE: WireType;
}

/// A type that can be decoded from protobuf wire format.
pub trait ProtoDecode: ProtoType + Default {
    /// Decode from `buf` into `dst`, following protobuf merging semantics.
    ///
    /// # Parameters
    /// * `buf`: The buffer to decode from (positioned at the value, after the key).
    /// * `dst`: The destination to decode into.
    /// * `offset`: Byte offset of this value in the message buffer.
    ///
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

// Re-export default checking trait
pub use default_check::IsProtoDefault;

// Re-export scalar types
pub use scalar::{Fixed32, Fixed64, Sfixed32, Sfixed64, Sint32, Sint64};

// Re-export length-delimited types
pub use delimited::{ProtoBytes, ProtoString};

// Re-export repeated field types
#[cfg(feature = "alloc")]
pub use repeated::{PackedIter, ProtoRepeated, Repeated, RepeatedDecodeIter, RepeatedIter};

// Re-export message types and helpers
pub use message::{
    decode_message_field, encode_message_field, encoded_message_field_len, skip_len_field,
    LazyMessage, ProtoMessage,
};

// Re-export oneof types and helpers
pub use oneof::{decode_oneof_field, encode_oneof_field, encoded_oneof_field_len, ProtoOneof};

// Re-export map field types
#[cfg(feature = "alloc")]
pub use map::{ProtoMap, ProtoMapKey};

// Re-export optimized packed decoding
#[cfg(feature = "alloc")]
pub use packed::PackedDecode;
