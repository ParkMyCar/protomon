//! Repeated field types and iterators.

use alloc::vec::Vec;

use super::{ProtoDecode, ProtoEncode, ProtoType};
use crate::error::DecodeErrorKind;
use crate::util::CastFrom;
use crate::wire::{self, WireType};

#[cfg(feature = "smallvec")]
use smallvec::SmallVec;

/// Storage for value offsets - uses SmallVec when available for inline storage.
#[cfg(feature = "smallvec")]
type OffsetVec = SmallVec<[u32; 8]>;

#[cfg(not(feature = "smallvec"))]
type OffsetVec = Vec<u32>;

/// Trait for repeated protobuf fields.
///
/// This trait provides a unified interface for repeated fields,
/// whether they are stored as `Vec<T>` or `Repeated<T>`. The derive macro
/// uses this trait to handle repeated fields uniformly.
pub trait ProtoRepeated: Default {
    /// Initialize for decoding with the message buffer and field tag.
    ///
    /// This must be called before `decode_into` for types that need the
    /// buffer/tag context (like `Repeated`). For `Vec`, this is a no-op.
    /// Takes a reference to avoid cloning for types that don't need the buffer.
    fn init_repeated(&mut self, msg_buf: &bytes::Bytes, tag: u32);

    /// Encode all elements with their field keys to the buffer.
    fn encode_repeated<B: bytes::BufMut>(&self, tag: u32, buf: &mut B);

    /// Returns the total encoded length including field keys.
    fn encoded_repeated_len(&self, tag: u32) -> usize;

    /// Returns the number of elements.
    fn repeated_len(&self) -> usize;

    /// Returns true if there are no elements.
    fn is_repeated_empty(&self) -> bool {
        self.repeated_len() == 0
    }
}

/// Repeated field that can be either lazily decoded or user-constructed.
///
/// # Lazy Variant
///
/// The protobuf wire spec denotes that repeated fields are stored as repeated
/// instances of `<tag><item>` in the encoded binary. The encoded values do not
/// need to be contiguous, e.g. the following is a valid encoding:
///
/// ```text
/// message GameResult {
///  string name = 2;
///  repeated scores e = 11;
/// }
///
/// 11: 99
/// 2: { "Parker" }
/// 11: 91
/// 11: 107
/// ```
///
/// During deserialization we skip over repeated field values, recording only
/// the count and minimum offset. When iterating, we scan from the minimum
/// offset to find all values with the matching tag.
///
/// Offsets stored point to the value (after the key has been decoded).
///
/// # Owned Variant
///
/// Users can construct a `Repeated` field with owned values for encoding:
///
/// ```ignore
/// let repeated: Repeated<i32> = Repeated::owned(vec![1, 2, 3]);
/// ```
pub enum Repeated<T: 'static> {
    /// Lazily decoded repeated field - references original buffer.
    Lazy {
        /// Buffer of the entire message this field belongs to.
        buf: bytes::Bytes,
        /// Value offsets (after key decoding) for O(1) iteration.
        offsets: OffsetVec,
        /// Running sum of encoded value lengths (excluding keys).
        values_len: u32,
        /// Marker for the element type.
        _marker: core::marker::PhantomData<T>,
    },
    /// User-constructed repeated field with owned values.
    Owned {
        /// The owned values.
        values: Vec<T>,
    },
}

impl<T: Clone + 'static> Clone for Repeated<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Lazy {
                buf,
                offsets,
                values_len,
                _marker,
            } => Self::Lazy {
                buf: buf.clone(),
                offsets: offsets.clone(),
                values_len: *values_len,
                _marker: *_marker,
            },
            Self::Owned { values } => Self::Owned {
                values: values.clone(),
            },
        }
    }
}

impl<T: 'static> core::fmt::Debug for Repeated<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Lazy { offsets, .. } => f
                .debug_struct("Repeated::Lazy")
                .field("len", &offsets.len())
                .finish(),
            Self::Owned { values } => f
                .debug_struct("Repeated::Owned")
                .field("len", &values.len())
                .finish(),
        }
    }
}

impl<T: 'static> Repeated<T> {
    /// Create a new empty Lazy repeated field wrapper.
    pub fn lazy(buf: bytes::Bytes) -> Self {
        Self::Lazy {
            buf,
            offsets: OffsetVec::new(),
            values_len: 0,
            _marker: core::marker::PhantomData,
        }
    }

    /// Create a new Owned repeated field from a Vec.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let repeated: Repeated<i32> = Repeated::owned(vec![1, 2, 3]);
    /// ```
    #[inline]
    pub fn owned(values: Vec<T>) -> Self {
        Self::Owned { values }
    }

    /// Returns the number of elements in this repeated field.
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::Lazy { offsets, .. } => offsets.len(),
            Self::Owned { values } => values.len(),
        }
    }

    /// Returns true if this repeated field has no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Lazy { offsets, .. } => offsets.is_empty(),
            Self::Owned { values } => values.is_empty(),
        }
    }
}

impl<T: ProtoDecode + Default + Clone + 'static> Repeated<T> {
    /// Returns an iterator over the repeated field elements.
    ///
    /// Works on both `Lazy` and `Owned` variants.
    pub fn iter(&self) -> RepeatedIter<'_, T> {
        match self {
            Self::Lazy { buf, offsets, .. } => {
                RepeatedIter::Decode(RepeatedDecodeIter::new(buf.clone(), offsets))
            }
            Self::Owned { values } => RepeatedIter::Owned(values.iter()),
        }
    }
}

impl<'a, T: ProtoDecode + Default + Clone + 'static> IntoIterator for &'a Repeated<T> {
    type Item = Result<T, DecodeErrorKind>;
    type IntoIter = RepeatedIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: 'static> Default for Repeated<T> {
    fn default() -> Self {
        Self::Lazy {
            buf: bytes::Bytes::new(),
            offsets: OffsetVec::new(),
            values_len: 0,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<T: ProtoType + 'static> ProtoType for Repeated<T> {
    // Use the element type's wire type.
    //
    // N.B. Non-packed repeated fields are encoded as multiples of `<tag><field>`.
    const WIRE_TYPE: WireType = T::WIRE_TYPE;
}

impl<T: ProtoType + 'static> ProtoDecode for Repeated<T> {
    /// Decode a single occurrence of a repeated field.
    ///
    /// Records the value offset and skips over the value in the buffer.
    ///
    /// # Errors
    ///
    /// Panics if `init_repeated` was not called first (i.e., if the buffer is empty).
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let Self::Lazy {
            buf: msg_buf,
            offsets,
            values_len,
            ..
        } = dst
        else {
            return Err(DecodeErrorKind::programming_error(
                "decode_into is not supported on Owned variant",
            ));
        };

        // Check that init_repeated was called
        if msg_buf.is_empty() {
            return Err(DecodeErrorKind::programming_error(
                "Repeated::init_repeated must be called before decode_into",
            ));
        }

        let before = buf.remaining();
        crate::wire::skip_field(T::WIRE_TYPE, buf)?;
        // `skip_field` will return an error if the value was larger then a u32.
        #[allow(clippy::as_conversions)]
        let value_len = (before - buf.remaining()) as u32;

        // Store the offset for O(1) iteration later
        #[allow(clippy::as_conversions)] // TODO(parker): change 'offset' to a u32.
        offsets.push(offset as u32);
        *values_len += value_len;

        Ok(())
    }
}

impl<T: ProtoType + ProtoEncode + ProtoDecode + Default + Clone + 'static> ProtoEncode
    for Repeated<T>
{
    /// Encode all values without field keys.
    ///
    /// The derive macro handles key encoding for each element.
    /// Silently skips any values that fail to decode.
    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        match self {
            Self::Lazy { .. } => {
                for value in self.iter().flatten() {
                    value.encode(buf);
                }
            }
            Self::Owned { values } => {
                for value in values {
                    value.encode(buf);
                }
            }
        }
    }

    /// Returns the encoded length of all values (not including field keys).
    #[inline]
    fn encoded_len(&self) -> usize {
        match self {
            // For Lazy, we tracked the length during decode.
            Self::Lazy { values_len, .. } => usize::cast_from(*values_len),
            // For Owned, iterate and sum.
            Self::Owned { values } => values.iter().map(|v| v.encoded_len()).sum(),
        }
    }
}

impl<T: ProtoType + ProtoEncode + ProtoDecode + Default + Clone + 'static> ProtoRepeated
    for Repeated<T>
{
    /// Initialize for decoding with the message buffer and field tag.
    #[inline]
    fn init_repeated(&mut self, msg_buf: &bytes::Bytes, _tag: u32) {
        *self = Self::Lazy {
            buf: msg_buf.clone(),
            offsets: OffsetVec::new(),
            values_len: 0,
            _marker: core::marker::PhantomData,
        };
    }

    /// Encode all elements with their field keys.
    #[inline]
    fn encode_repeated<B: bytes::BufMut>(&self, tag: u32, buf: &mut B) {
        match self {
            Self::Lazy { .. } => {
                for value in self.iter().flatten() {
                    wire::encode_key(T::WIRE_TYPE, tag, buf);
                    value.encode(buf);
                }
            }
            Self::Owned { values } => {
                for value in values {
                    wire::encode_key(T::WIRE_TYPE, tag, buf);
                    value.encode(buf);
                }
            }
        }
    }

    #[inline]
    fn encoded_repeated_len(&self, tag: u32) -> usize {
        let count = self.len();
        if count == 0 {
            return 0;
        }
        let key_len = wire::encoded_key_len(tag);
        match self {
            // For Lazy, we tracked values_len during decode
            Self::Lazy { values_len, .. } => count * key_len + usize::cast_from(*values_len),
            // For Owned, iterate and sum
            Self::Owned { values } => values.iter().map(|v| key_len + v.encoded_len()).sum(),
        }
    }

    #[inline]
    fn repeated_len(&self) -> usize {
        self.len()
    }
}

/// Iterator over repeated field elements.
///
/// This enum wraps both lazy decoding (from `Repeated::Lazy`) and owned iteration
/// (from `Repeated::Owned`), providing a unified interface that exposes decode errors.
pub enum RepeatedIter<'a, T: 'static> {
    /// Decoding iterator for lazily decoded repeated fields.
    Decode(RepeatedDecodeIter<'a, T>),
    /// Slice iterator for user-constructed repeated fields.
    Owned(core::slice::Iter<'a, T>),
}

impl<T: ProtoDecode + Default + Clone + 'static> Iterator for RepeatedIter<'_, T> {
    type Item = Result<T, DecodeErrorKind>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Decode(iter) => iter.next(),
            Self::Owned(iter) => iter.next().cloned().map(Ok),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Decode(iter) => iter.size_hint(),
            Self::Owned(iter) => iter.size_hint(),
        }
    }
}

impl<T: ProtoDecode + Default + Clone + 'static> ExactSizeIterator for RepeatedIter<'_, T> {}

/// Iterator for lazily decoding repeated fields from a buffer.
///
/// Uses pre-computed offsets for O(1) element access.
pub struct RepeatedDecodeIter<'a, T> {
    /// Buffer of the entire message this field belongs to.
    buf: bytes::Bytes,
    /// Iterator over stored offsets.
    offset_iter: core::slice::Iter<'a, u32>,

    _marker: core::marker::PhantomData<T>,
}

impl<'a, T> RepeatedDecodeIter<'a, T> {
    /// Create a new [`RepeatedDecodeIter`].
    fn new(buf: bytes::Bytes, offsets: &'a OffsetVec) -> Self {
        Self {
            buf,
            offset_iter: offsets.iter(),
            _marker: core::marker::PhantomData,
        }
    }
}

impl<T: ProtoDecode + Default> Iterator for RepeatedDecodeIter<'_, T> {
    type Item = Result<T, DecodeErrorKind>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let &offset = self.offset_iter.next()?;
        let value_offset = usize::cast_from(offset);

        // Use Bytes::slice() to maintain zero-copy semantics for nested decoding
        let mut buf_slice = self.buf.slice(value_offset..);
        let mut value = T::default();
        let result = T::decode_into(&mut buf_slice, &mut value, value_offset);

        Some(result.map(|()| value))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.offset_iter.size_hint()
    }
}

impl<T: ProtoDecode + Default> ExactSizeIterator for RepeatedDecodeIter<'_, T> {}

impl<T: ProtoType> ProtoType for Vec<T> {
    const WIRE_TYPE: WireType = T::WIRE_TYPE;
}

impl<T: ProtoDecode> ProtoDecode for Vec<T> {
    /// Decode a single occurrence of a repeated field and push to the Vec.
    #[inline(always)]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let mut value = T::default();
        T::decode_into(buf, &mut value, offset)?;
        dst.push(value);
        Ok(())
    }
}

impl<T: ProtoType + ProtoEncode> ProtoEncode for Vec<T> {
    /// Encode all values without field keys.
    ///
    /// The derive macro handles key encoding for each element.
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        for value in self {
            value.encode(buf);
        }
    }

    /// Returns the encoded length of all values (not including field keys).
    fn encoded_len(&self) -> usize {
        self.iter().map(|v| v.encoded_len()).sum()
    }
}

impl<T: ProtoType + ProtoEncode> ProtoRepeated for Vec<T> {
    /// No-op for Vec - it doesn't need buffer/tag context.
    #[inline]
    fn init_repeated(&mut self, _msg_buf: &bytes::Bytes, _tag: u32) {
        // Vec doesn't need initialization context - no clone happens
    }

    fn encode_repeated<B: bytes::BufMut>(&self, tag: u32, buf: &mut B) {
        for value in self {
            wire::encode_key(T::WIRE_TYPE, tag, buf);
            value.encode(buf);
        }
    }

    fn encoded_repeated_len(&self, tag: u32) -> usize {
        if self.is_empty() {
            return 0;
        }
        let key_len = wire::encoded_key_len(tag);
        self.iter().map(|v| key_len + v.encoded_len()).sum()
    }

    fn repeated_len(&self) -> usize {
        self.len()
    }
}

/// Decode a repeated field element, handling both packed and unpacked encodings.
///
/// This function should be called instead of `ProtoDecode::decode_into` for
/// repeated `Vec<T>` fields, as it handles the case where the encoder used
/// packed encoding (wire type LEN) for scalar types.
///
/// # Packed vs Unpacked
///
/// - **Unpacked**: Each element has its own `<tag><value>` pair in the wire format.
///   The wire type matches the element type.
/// - **Packed**: All elements are concatenated into a single `<tag><length><values...>`.
///   The wire type is LEN, regardless of element type.
///
/// This function detects packed encoding by checking if the wire type is LEN
/// when the element's wire type is not LEN (i.e., a scalar type).
#[inline]
pub fn decode_repeated_into<T, B>(
    wire_type: WireType,
    buf: &mut B,
    dst: &mut Vec<T>,
    _offset: usize,
) -> Result<(), DecodeErrorKind>
where
    T: ProtoType + ProtoDecode + Default,
    B: bytes::Buf,
{
    // Check for packed encoding: wire type is LEN but element type is not LEN
    if wire_type == WireType::Len && T::WIRE_TYPE != WireType::Len {
        // Packed encoding - decode length, then decode all values
        let len = wire::decode_len(buf)?;
        if buf.remaining() < len {
            return Err(DecodeErrorKind::unexpected_end_of_buffer());
        }
        // Read the packed data into a slice and decode values
        let data = buf.copy_to_bytes(len);
        let mut slice = &data[..];
        while !slice.is_empty() {
            let mut value = T::default();
            T::decode_into(&mut slice, &mut value, 0)?;
            dst.push(value);
        }
    } else {
        // Regular (unpacked) encoding - decode single value
        let mut value = T::default();
        T::decode_into(buf, &mut value, 0)?;
        dst.push(value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::string::{String, ToString};
    use alloc::vec;

    use super::super::{ProtoEncode, ProtoString};
    use super::*;
    use crate::wire::WireType;

    fn build_test_message() -> Vec<u8> {
        use crate::wire::encode_key;

        let mut buf = Vec::new();

        // Field 1: int32 = 42
        encode_key(WireType::Varint, 1, &mut buf);
        42u32.encode(&mut buf);

        // Field 2: string = "hello"
        encode_key(WireType::Len, 2, &mut buf);
        ProtoString::from("hello").encode(&mut buf);

        // Field 1: int32 = 99
        encode_key(WireType::Varint, 1, &mut buf);
        99u32.encode(&mut buf);

        // Field 2: string = "world"
        encode_key(WireType::Len, 2, &mut buf);
        ProtoString::from("world").encode(&mut buf);

        // Field 2: string = "!"
        encode_key(WireType::Len, 2, &mut buf);
        ProtoString::from("!").encode(&mut buf);

        buf
    }

    #[test]
    fn test_repeated_decode() {
        use crate::wire::decode_key;
        use bytes::Buf;

        let buf = build_test_message();
        let bytes_buf = bytes::Bytes::from(buf);

        // Simulate decoding using Default + init_repeated
        let mut repeated: Repeated<ProtoString> = Repeated::default();
        repeated.init_repeated(&bytes_buf, 2);
        let mut slice = &bytes_buf[..];

        while slice.has_remaining() {
            let (wire_type, tag) = decode_key(&mut slice).unwrap();
            // Value offset is now (after key decode)
            let value_offset = bytes_buf.len() - slice.len();

            if tag == 2 {
                // Use decode_into - it records offset and skips the field
                <Repeated<ProtoString> as ProtoDecode>::decode_into(
                    &mut slice,
                    &mut repeated,
                    value_offset,
                )
                .unwrap();
            } else {
                crate::wire::skip_field(wire_type, &mut slice).unwrap();
            }
        }

        assert_eq!(repeated.len(), 3);
        assert!(!repeated.is_empty());

        let strings: Vec<String> = repeated
            .iter()
            .map(|r| r.unwrap().as_str().to_string())
            .collect();
        assert_eq!(strings, vec!["hello", "world", "!"]);

        // Second iteration works too
        let strings2: Vec<String> = repeated
            .iter()
            .map(|r| r.unwrap().as_str().to_string())
            .collect();
        assert_eq!(strings2, vec!["hello", "world", "!"]);
    }

    #[test]
    fn test_repeated_encode() {
        use crate::wire::decode_key;
        use bytes::Buf;

        let buf = build_test_message();
        let bytes_buf = bytes::Bytes::from(buf);

        // Decode the repeated field
        let mut repeated: Repeated<ProtoString> = Repeated::default();
        repeated.init_repeated(&bytes_buf, 2);
        let mut slice = &bytes_buf[..];

        while slice.has_remaining() {
            let (wire_type, tag) = decode_key(&mut slice).unwrap();
            let value_offset = bytes_buf.len() - slice.len();

            if tag == 2 {
                <Repeated<ProtoString> as ProtoDecode>::decode_into(
                    &mut slice,
                    &mut repeated,
                    value_offset,
                )
                .unwrap();
            } else {
                crate::wire::skip_field(wire_type, &mut slice).unwrap();
            }
        }

        // Now encode it back (values only, no keys - derive macro handles keys)
        let mut encoded = Vec::new();
        repeated.encode(&mut encoded);

        // Verify encoded_len matches
        assert_eq!(encoded.len(), repeated.encoded_len());

        // Decode the encoded buffer directly (no keys expected)
        let mut decoded_strings = Vec::new();
        let mut slice = &encoded[..];

        while slice.has_remaining() {
            let mut s = ProtoString::default();
            ProtoString::decode_into(&mut slice, &mut s, 0).unwrap();
            decoded_strings.push(s.as_str().to_string());
        }

        assert_eq!(decoded_strings, vec!["hello", "world", "!"]);
    }

    #[test]
    fn test_repeated_encode_empty() {
        let mut repeated: Repeated<ProtoString> = Repeated::default();
        repeated.init_repeated(&bytes::Bytes::new(), 1);

        assert_eq!(repeated.encoded_len(), 0);

        let mut encoded = Vec::new();
        repeated.encode(&mut encoded);
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_repeated_owned_encode() {
        use bytes::Buf;

        // Create an Owned repeated field from a vector
        let values = vec![1i32, 2, 3, 4, 5];
        let repeated: Repeated<i32> = Repeated::owned(values.clone());

        // Test len() and is_empty()
        assert_eq!(repeated.len(), 5);
        assert!(!repeated.is_empty());

        // Test iter() returns the values (wrapped in Ok)
        let collected: Vec<i32> = repeated.iter().map(|r| r.unwrap()).collect();
        assert_eq!(collected, values);

        // Test encode (now encodes values only, no keys)
        let mut encoded = Vec::new();
        repeated.encode(&mut encoded);

        // Verify encoded_len matches
        assert_eq!(encoded.len(), repeated.encoded_len());

        // Decode the values directly (no keys)
        let mut decoded_values = Vec::new();
        let mut slice = &encoded[..];

        while slice.has_remaining() {
            let mut value = 0i32;
            i32::decode_into(&mut slice, &mut value, 0).unwrap();
            decoded_values.push(value);
        }

        assert_eq!(decoded_values, values);
    }

    #[test]
    fn test_repeated_owned_empty() {
        let repeated: Repeated<i32> = Repeated::owned(vec![]);

        assert_eq!(repeated.len(), 0);
        assert!(repeated.is_empty());
        assert_eq!(repeated.encoded_len(), 0);

        let mut encoded = Vec::new();
        repeated.encode(&mut encoded);
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_repeated_owned_clone() {
        let values = vec![10i32, 20, 30];
        let repeated: Repeated<i32> = Repeated::owned(values.clone());

        // Clone the repeated field
        let cloned = repeated.clone();

        // Both should encode identically (values only, no keys)
        let mut encoded1 = Vec::new();
        let mut encoded2 = Vec::new();
        repeated.encode(&mut encoded1);
        cloned.encode(&mut encoded2);

        assert_eq!(encoded1, encoded2);
    }
}
