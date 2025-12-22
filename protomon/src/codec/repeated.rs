//! Repeated field types and iterators.

use super::{ProtoDecode, ProtoEncode, ProtoType};
use crate::error::DecodeErrorKind;
use crate::wire::{self, WireType};
use core::num::NonZeroU32;

/// Trait for encoding repeated protobuf fields.
///
/// This trait provides a unified interface for encoding repeated fields,
/// whether they are stored as `Vec<T>` or `Repeated<T>`. The derive macro
/// uses this trait to encode repeated fields uniformly.
pub trait ProtoRepeated {
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

/// Iterator over packed repeated scalar fields.
///
/// Packed repeated fields encode all elements contiguously in a single
/// length-delimited blob. This iterator decodes elements lazily.
pub struct PackedIter<T> {
    buf: bytes::Bytes,
    offset: usize,
    _marker: core::marker::PhantomData<T>,
}

impl<T> PackedIter<T> {
    /// Create a new iterator over packed repeated elements.
    pub fn new(buf: bytes::Bytes) -> Self {
        Self {
            buf,
            offset: 0,
            _marker: core::marker::PhantomData,
        }
    }

    /// Returns the remaining bytes that haven't been iterated yet.
    pub fn remaining_bytes(&self) -> &[u8] {
        &self.buf[self.offset..]
    }
}

impl<T: ProtoDecode + Default> Iterator for PackedIter<T> {
    type Item = Result<T, DecodeErrorKind>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.buf.len() {
            return None;
        }
        let mut slice = &self.buf[self.offset..];
        let start_len = slice.len();
        let mut value = T::default();
        match T::decode_into(&mut slice, &mut value, self.offset) {
            Ok(()) => {
                self.offset += start_len - slice.len();
                Some(Ok(value))
            }
            Err(e) => Some(Err(e)),
        }
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
/// let repeated: Repeated<i32> = Repeated::owned(vec![1, 2, 3].into_iter());
/// ```
pub enum Repeated<T: 'static> {
    /// Lazily decoded repeated field - references original buffer.
    Lazy {
        /// Buffer of the entire message this field belongs to.
        buf: bytes::Bytes,
        /// The tag number for the repeated field (used for scan mode iteration).
        tag_num: u32,
        /// Number of repeated field occurrences.
        count: u32,
        /// Minimum value offset seen (after key). Never 0 since there's always a key.
        min_offset: Option<NonZeroU32>,
        /// Running sum of encoded value lengths (excluding keys).
        values_len: u32,
        /// Marker for the element type.
        _marker: core::marker::PhantomData<T>,
    },
    /// User-constructed repeated field with owned values.
    Owned {
        /// Iterator over the values.
        iter: Box<dyn CloneableIterator<T>>,
    },
}

impl<T: 'static> Clone for Repeated<T> {
    fn clone(&self) -> Self {
        match self {
            lazy @ Self::Lazy { .. } => lazy.clone(),
            Self::Owned { iter } => Self::Owned {
                iter: iter.clone_box(),
            },
        }
    }
}

impl<T: 'static> Repeated<T> {
    /// Create a new empty Lazy repeated field wrapper.
    pub fn lazy(buf: bytes::Bytes, tag_num: u32) -> Self {
        Self::Lazy {
            buf,
            tag_num,
            count: 0,
            min_offset: None,
            values_len: 0,
            _marker: core::marker::PhantomData,
        }
    }

    /// Create a new Owned repeated field from an iterator.
    ///
    /// The iterator must implement `ExactSizeIterator` so we can efficiently
    /// determine the field count.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let repeated: Repeated<i32> = Repeated::owned(vec![1, 2, 3].into_iter());
    /// ```
    pub fn owned<I>(iter: I) -> Self
    where
        I: Iterator<Item = T> + ExactSizeIterator + Clone + 'static,
    {
        Self::Owned {
            iter: Box::new(iter),
        }
    }

    /// Returns the number of elements in this repeated field.
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::Lazy { count, .. } => *count as usize,
            Self::Owned { iter } => iter.len(),
        }
    }

    /// Returns true if this repeated field has no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Lazy { count, .. } => *count == 0,
            Self::Owned { iter } => iter.is_empty(),
        }
    }
}

impl<T: ProtoDecode + Default + 'static> Repeated<T> {
    /// Returns an iterator over the repeated field elements.
    ///
    /// Works on both `Lazy` and `Owned` variants.
    pub fn iter(&self) -> RepeatedIter<'_, T> {
        match self {
            Self::Lazy {
                buf,
                tag_num,
                count,
                min_offset,
                ..
            } => {
                let offset = match min_offset {
                    Some(o) => o.get() as usize,
                    // Invariant: If we don't have an offset then we must not have any elements.
                    None => {
                        assert_eq!(*count, 0);
                        0
                    }
                };
                RepeatedIter::Decode(RepeatedDecodeIter::new(
                    buf.clone(),
                    *tag_num,
                    *count as usize,
                    offset,
                ))
            }
            Self::Owned { iter } => RepeatedIter::Owned(iter.clone_box()),
        }
    }
}

impl<'a, T: ProtoDecode + Default + 'static> IntoIterator for &'a Repeated<T> {
    type Item = Result<T, DecodeErrorKind>;
    type IntoIter = RepeatedIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: ProtoType + 'static> ProtoType for Repeated<T> {
    // Use the element type's wire type.
    //
    // N.B. Non-packed repeated fields are encoded as multiples of `<tag><field>`.
    const WIRE_TYPE: WireType = T::WIRE_TYPE;
}

impl<T: ProtoType + 'static> ProtoDecode for Repeated<T> {
    #[inline]
    fn init<B: bytes::Buf>(msg_buf: B, tag: u32) -> Self {
        Self::Lazy {
            buf: bytes::Bytes::copy_from_slice(msg_buf.chunk()),
            tag_num: tag,
            count: 0,
            min_offset: None,
            values_len: 0,
            _marker: core::marker::PhantomData,
        }
    }

    /// Decode a single occurrence of a repeated field.
    ///
    /// Records the value offset and skips over the value in the buffer.
    ///
    /// # Panics
    ///
    /// Panics if called on an `Owned` variant.
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let Self::Lazy {
            count,
            min_offset,
            values_len,
            ..
        } = dst
        else {
            return Err(DecodeErrorKind::ProgrammingError {
                reason: "decode_into is not supported on Owned variant",
            });
        };

        let before = buf.remaining();
        crate::wire::skip_field(T::WIRE_TYPE, buf)?;
        let value_len = (before - buf.remaining()) as u32;

        // Value offsets are always > 0 because there is at least a key before this.
        let offset_nz = NonZeroU32::new(offset as u32).expect("value offset cannot be 0");
        *min_offset = Some(match *min_offset {
            Some(current) => current.min(offset_nz),
            None => offset_nz,
        });
        *count += 1;
        *values_len += value_len;

        Ok(())
    }
}

impl<T: ProtoType + ProtoEncode + ProtoDecode + Default + 'static> ProtoEncode for Repeated<T> {
    /// Encode all values without field keys.
    ///
    /// The derive macro handles key encoding for each element.
    /// Silently skips any values that fail to decode.
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        for result in self.iter() {
            if let Ok(value) = result {
                value.encode(buf);
            }
        }
    }

    /// Returns the encoded length of all values (not including field keys).
    fn encoded_len(&self) -> usize {
        match self {
            // For Lazy, we tracked the length during decode
            Self::Lazy { values_len, .. } => *values_len as usize,
            // For Owned, we must iterate and sum
            Self::Owned { iter } => iter.clone_box().map(|v| v.encoded_len()).sum(),
        }
    }
}

impl<T: ProtoType + ProtoEncode + ProtoDecode + Default + 'static> ProtoRepeated for Repeated<T> {
    /// Encode all elements with their field keys.
    fn encode_repeated<B: bytes::BufMut>(&self, tag: u32, buf: &mut B) {
        for result in self.iter() {
            if let Ok(value) = result {
                wire::encode_key(T::WIRE_TYPE, tag, buf);
                value.encode(buf);
            }
        }
    }

    fn encoded_repeated_len(&self, tag: u32) -> usize {
        let count = self.len();
        if count == 0 {
            return 0;
        }
        let key_len = wire::encoded_key_len(tag);
        match self {
            // For Lazy, we tracked values_len during decode
            Self::Lazy { values_len, .. } => count * key_len + *values_len as usize,
            // For Owned, we must iterate and sum
            Self::Owned { iter } => iter
                .clone_box()
                .map(|v| key_len + v.encoded_len())
                .sum(),
        }
    }

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
    /// Owned iterator for user-constructed repeated fields.
    Owned(Box<dyn CloneableIterator<T> + 'a>),
}

impl<T: ProtoDecode + Default + 'static> Iterator for RepeatedIter<'_, T> {
    type Item = Result<T, DecodeErrorKind>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Decode(iter) => iter.next(),
            Self::Owned(iter) => iter.next().map(Ok),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Decode(iter) => iter.size_hint(),
            Self::Owned(iter) => {
                let len = iter.len();
                (len, Some(len))
            }
        }
    }
}

impl<T: ProtoDecode + Default + 'static> ExactSizeIterator for RepeatedIter<'_, T> {}

/// Iterator for lazily decoding repeated fields from a buffer.
///
/// Scans through the message buffer starting at min_offset to find
/// all occurrences of the repeated field.
pub struct RepeatedDecodeIter<'a, T> {
    /// Buffer of the entire message this field belongs to.
    buf: bytes::Bytes,
    /// The tag number for the repeated field.
    tag_num: u32,
    /// Remaining elements to iterate.
    remaining: usize,
    /// Current byte offset in the buffer.
    offset: usize,
    /// Whether we've started iterating (first element is at min_offset).
    started: bool,

    _marker: core::marker::PhantomData<&'a T>,
}

impl<T> RepeatedDecodeIter<'_, T> {
    /// Create a new [`RepeatedDecodeIter`].
    fn new(buf: bytes::Bytes, tag_num: u32, count: usize, min_offset: usize) -> Self {
        Self {
            buf,
            tag_num,
            remaining: count,
            offset: min_offset,
            started: false,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<T: ProtoDecode + Default> Iterator for RepeatedDecodeIter<'_, T> {
    type Item = Result<T, DecodeErrorKind>;

    fn next(&mut self) -> Option<Self::Item> {
        // Exit early if we've already iterated over everything.
        if self.remaining == 0 {
            return None;
        }

        // Find the byte offset positioned right after the key (value offset).
        let value_offset = if !self.started {
            // First offset points directly to a value.
            self.started = true;
            self.offset
        } else {
            // Scan for the next key.
            Self::scan_for_field(&self.buf, self.tag_num, &mut self.offset)?
        };

        self.remaining = self.remaining.saturating_sub(1);

        // Decode the value.
        let mut slice = &self.buf[value_offset..];
        let mut value = T::default();
        let result = T::decode_into(&mut slice, &mut value, value_offset);
        let after_value = self.buf.len() - slice.len();

        // Update offset to point past this field (for next scan)
        self.offset = after_value;

        Some(result.map(|()| value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T: ProtoDecode + Default> RepeatedDecodeIter<'_, T> {
    /// Scan through buffer starting at byte_offset to find next matching field.
    fn scan_for_field(buf: &bytes::Bytes, tag_num: u32, byte_offset: &mut usize) -> Option<usize> {
        use crate::wire::{decode_key, skip_field};

        loop {
            if *byte_offset >= buf.len() {
                return None;
            }

            let mut slice = &buf[*byte_offset..];
            let (wire_type, tag) = match decode_key(&mut slice) {
                Ok(k) => k,
                Err(_) => return None,
            };

            let value_offset = buf.len() - slice.len();

            if tag == tag_num {
                return Some(value_offset);
            }

            // Skip non-matching field
            *byte_offset = value_offset;
            let mut skip_slice = &buf[*byte_offset..];
            let skip_start = skip_slice.len();
            if skip_field(wire_type, &mut skip_slice).is_err() {
                return None;
            }
            *byte_offset += skip_start - skip_slice.len();
        }
    }
}

impl<T: ProtoDecode + Default> ExactSizeIterator for RepeatedDecodeIter<'_, T> {}

impl<T: ProtoType> ProtoType for Vec<T> {
    const WIRE_TYPE: WireType = T::WIRE_TYPE;
}

impl<T: ProtoDecode + Default> ProtoDecode for Vec<T> {
    #[inline]
    fn init<B: bytes::Buf>(_msg_buf: B, _tag: u32) -> Self {
        Vec::new()
    }

    /// Decode a single occurrence of a repeated field and push to the Vec.
    #[inline]
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
        self.iter()
            .map(|v| key_len + v.encoded_len())
            .sum()
    }

    fn repeated_len(&self) -> usize {
        self.len()
    }
}

/// Object-safe trait for cloneable, exact-size iterators.
///
/// This trait allows storing iterators in a `Box<dyn CloneableIterator<T>>` while
/// still being able to clone them (via `clone_box`) and get their length.
/// This is needed because `Clone` and `ExactSizeIterator` are not object-safe.
pub trait CloneableIterator<T>: Iterator<Item = T> {
    /// Clone this iterator into a new boxed iterator.
    fn clone_box(&self) -> Box<dyn CloneableIterator<T>>;

    /// Returns the exact remaining length of the iterator.
    fn len(&self) -> usize;

    /// Returns true if the iterator has no more elements.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T, I> CloneableIterator<T> for I
where
    I: Iterator<Item = T> + ExactSizeIterator + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn CloneableIterator<T>> {
        Box::new(self.clone())
    }

    fn len(&self) -> usize {
        ExactSizeIterator::len(self)
    }
}

#[cfg(test)]
mod tests {
    use super::super::scalar::Fixed32;
    use super::super::{ProtoEncode, ProtoString};
    use super::*;
    use crate::wire::WireType;

    #[test]
    fn test_packed_iter_u32() {
        let mut buf = Vec::new();
        1u32.encode(&mut buf);
        127u32.encode(&mut buf);
        128u32.encode(&mut buf);
        300u32.encode(&mut buf);

        let iter: PackedIter<u32> = PackedIter::new(bytes::Bytes::from(buf));
        let values: Vec<u32> = iter.map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![1, 127, 128, 300]);
    }

    #[test]
    fn test_packed_iter_fixed32() {
        let mut buf = Vec::new();
        Fixed32(1).encode(&mut buf);
        Fixed32(2).encode(&mut buf);
        Fixed32(3).encode(&mut buf);

        let iter: PackedIter<Fixed32> = PackedIter::new(bytes::Bytes::from(buf));
        let values: Vec<Fixed32> = iter.map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![Fixed32(1), Fixed32(2), Fixed32(3)]);
    }

    #[test]
    fn test_packed_iter_empty() {
        let iter: PackedIter<u32> = PackedIter::new(bytes::Bytes::new());
        let values: Vec<u32> = iter.map(|r| r.unwrap()).collect();
        assert!(values.is_empty());
    }

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

        // Simulate decoding using ProtoDecode
        let mut repeated: Repeated<ProtoString> =
            <Repeated<ProtoString> as ProtoDecode>::init(bytes_buf.clone(), 2);
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
        let mut repeated: Repeated<ProtoString> =
            <Repeated<ProtoString> as ProtoDecode>::init(bytes_buf.clone(), 2);
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
        let repeated: Repeated<ProtoString> =
            <Repeated<ProtoString> as ProtoDecode>::init(bytes::Bytes::new(), 1);

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
        let repeated: Repeated<i32> = Repeated::owned(values.clone().into_iter());

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
        let repeated: Repeated<i32> = Repeated::owned(std::iter::empty());

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
        let repeated: Repeated<i32> = Repeated::owned(values.clone().into_iter());

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
