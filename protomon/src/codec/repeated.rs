//! Repeated field types and iterators.

use super::{ProtoDecode, ProtoEncode, ProtoType};
use crate::error::DecodeErrorKind;
use crate::wire::WireType;
use core::num::NonZeroU32;

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

/// Lazily decoded `repeated` field.
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
/// During deserialization we could allocate a [`Vec`] and build a collection
/// of these items, but that's most likely wasteful. Generally you never need
/// to access a field, or you only access it once. So instead we make this type
/// generic over a [`RepeatedStorage`].
///
/// Offsets stored point to the value (after the key has been decoded).
#[derive(Clone)]
pub struct Repeated<T, S: RepeatedStorage = BasicStorage> {
    /// Buffer of the entire message this field belongs to.
    buf: bytes::Bytes,
    /// The tag number for the repeated field (used for scan mode).
    tag_num: u32,
    /// Record of value offsets (after key) encountered during initial deserialization.
    storage: S,
    /// Running sum of encoded value lengths (excluding keys).
    values_len: u32,

    _marker: core::marker::PhantomData<T>,
}

impl<T, S: RepeatedStorage> Repeated<T, S> {
    /// Create a new empty Repeated field wrapper.
    pub fn new(buf: bytes::Bytes, tag_num: u32) -> Self {
        Self {
            buf,
            tag_num,
            storage: S::default(),
            values_len: 0,
            _marker: core::marker::PhantomData,
        }
    }

    /// Returns the number of elements in this repeated field.
    #[inline]
    pub fn len(&self) -> usize {
        self.storage.count()
    }

    /// Returns true if this repeated field has no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.storage.count() == 0
    }

    /// Returns an iterator over the repeated field elements.
    pub fn iter(&self) -> RepeatedIter<'_, T> {
        let mode = match self.storage.offsets() {
            Some(offsets) => RepeatedIterMode::IndexOffsets { offsets, index: 0 },
            None => {
                let offset = match self.storage.min_offset() {
                    Some(o) => o.get() as usize,
                    // Invariant: If we don't have an offset then we must not have any elements.
                    None => {
                        assert_eq!(self.storage.count(), 0);
                        0
                    }
                };
                RepeatedIterMode::Scan {
                    offset,
                    started: false,
                }
            }
        };
        RepeatedIter::new(self.buf.clone(), self.tag_num, self.storage.count(), mode)
    }
}

impl<'a, T: ProtoDecode + Default, S: RepeatedStorage> IntoIterator for &'a Repeated<T, S> {
    type Item = Result<T, DecodeErrorKind>;
    type IntoIter = RepeatedIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: ProtoType, S: RepeatedStorage> ProtoType for Repeated<T, S> {
    // Use the element type's wire type.
    //
    // N.B. Non-packed repeated fields are encoded as multiples of `<tag><field>`.
    const WIRE_TYPE: WireType = T::WIRE_TYPE;
}

impl<T: ProtoType, S: RepeatedStorage> ProtoDecode for Repeated<T, S> {
    #[inline]
    fn init<B: bytes::Buf>(msg_buf: B, tag: u32) -> Self {
        Self {
            buf: bytes::Bytes::copy_from_slice(msg_buf.chunk()),
            tag_num: tag,
            storage: S::default(),
            values_len: 0,
            _marker: core::marker::PhantomData,
        }
    }

    /// Decode a single occurrence of a repeated field.
    ///
    /// Records the value offset and skips over the value in the buffer.
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        let before = buf.remaining();
        crate::wire::skip_field(T::WIRE_TYPE, buf)?;
        let value_len = (before - buf.remaining()) as u32;

        dst.storage.merge_into(offset as u32);
        dst.values_len += value_len;
        Ok(())
    }
}

impl<T: ProtoType + ProtoEncode + ProtoDecode + Default, S: RepeatedStorage> ProtoEncode
    for Repeated<T, S>
{
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        use crate::wire::encode_key;

        for item in self.iter() {
            if let Ok(value) = item {
                encode_key(T::WIRE_TYPE, self.tag_num, buf);
                value.encode(buf);
            }
        }
    }

    fn encoded_len(&self) -> usize {
        let count = self.storage.count();
        if count == 0 {
            return 0;
        }

        // Each element needs: <key (tag + wire_type as varint)><value>
        let key_len = crate::wire::encoded_key_len(self.tag_num);
        key_len * count + self.values_len as usize
    }
}

/// Iterator over unpacked repeated fields.
///
/// Supports two modes based on [`RepeatedStorage`]:
/// * Indexed mode (with offsets): jumps directly to stored value offsets
/// * BasicScan mode: first item decoded directly from min_offset, rest scanned
///
pub struct RepeatedIter<'a, T> {
    /// Buffer of the entire message this field belongs to.
    buf: bytes::Bytes,
    /// The tag number for the repeated field (used for BasicScan mode).
    tag_num: u32,
    /// Remaining elements to iterate.
    remaining: usize,
    /// Iteration mode.
    mode: RepeatedIterMode<'a>,

    _marker: core::marker::PhantomData<T>,
}

enum RepeatedIterMode<'a> {
    /// Scan for the remaining values, starting at `offset`.
    Scan { offset: usize, started: bool },
    /// Stored value offsets (after key) from the beginning of the buffer.
    IndexOffsets { offsets: &'a [u32], index: usize },
}

impl<'a, T> RepeatedIter<'a, T> {
    /// Create a new [`RepeatedIter`].
    fn new(buf: bytes::Bytes, tag_num: u32, count: usize, mode: RepeatedIterMode<'a>) -> Self {
        Self {
            buf,
            tag_num,
            remaining: count,
            mode,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<'a, T: ProtoDecode + Default> Iterator for RepeatedIter<'a, T> {
    type Item = Result<T, DecodeErrorKind>;

    fn next(&mut self) -> Option<Self::Item> {
        // Exit early if we've already iterated over everything.
        if self.remaining == 0 {
            return None;
        }

        // Find the byte offset positioned right after the key (value offset).
        let value_offset = match &mut self.mode {
            // We have stored value offsets, jump directly to the value.
            RepeatedIterMode::IndexOffsets { offsets, index } => {
                let offset = *offsets.get(*index)? as usize;
                *index += 1;
                offset
            }
            // Start scanning from the first element.
            RepeatedIterMode::Scan { offset, started } => {
                // First offset points directly to a value, so we can return the offset.
                if !*started {
                    *started = true;
                    *offset
                } else {
                    // Otherwise we must scan for the next key.
                    Self::scan_for_field(&self.buf, self.tag_num, offset)?
                }
            }
        };

        self.remaining = self.remaining.saturating_sub(1);

        // Decode the value.
        let mut slice = &self.buf[value_offset..];
        let mut value = T::default();
        let result = T::decode_into(&mut slice, &mut value, value_offset);
        let after_value = self.buf.len() - slice.len();

        // Update offset to point past this field (for next scan)
        if let RepeatedIterMode::Scan { offset, .. } = &mut self.mode {
            *offset = after_value;
        }

        Some(result.map(|()| value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a, T: ProtoDecode + Default> RepeatedIter<'a, T> {
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

impl<'a, T: ProtoDecode + Default> ExactSizeIterator for RepeatedIter<'a, T> {}

/// Storage for recording the repeated fields we see during deserialization.
///
/// When deserializing a message we must visit every field. We can skip
/// deserializing the instance of the field, but we must iterate to find all
/// the _other_ fields in the message. To later aid deserialization of these
/// repeated values, we store some metadata about them.
///
/// For minimal-allocation deserialization, use [`BasicStorage`] which stores
/// just the count and minimum offset. Alternatively [`Vec<u32>`] and [`SmallVec`]
/// implement [`RepeatedStorage`], and they maintain offsets for each value.
///
/// [`SmallVec`]: [`smallvec::SmallVec`].
pub trait RepeatedStorage: Default + Clone {
    /// Record a repeated field occurrence at the given byte offset.
    fn merge_into(&mut self, offset: u32);

    /// Returns the number of elements.
    fn count(&self) -> usize;

    /// Returns the stored offsets, if available.
    /// Returns `None` for count-only storage.
    fn offsets(&self) -> Option<&[u32]>;

    /// Returns the minimum value offset seen, for scan mode optimization.
    /// Since this is a value offset (after the key), it's never 0.
    fn min_offset(&self) -> Option<NonZeroU32>;
}

/// Zero allocation storage for repeated fields.
///
/// This stores the number of values for a repeated field, and the offset of
/// the first value. The idea here is most protobuf implementations will encode
/// the values of a repeated field one after another. So by storing the first
/// offset we can very quickly jump to where all of the values are in memory.
#[derive(Debug, Clone, Copy, Default)]
pub struct BasicStorage {
    /// Number of repeated field occurrences.
    count: u32,
    /// Minimum value offset seen (after key). Never 0 since there's always a key.
    min_offset: Option<NonZeroU32>,
}

impl RepeatedStorage for BasicStorage {
    #[inline]
    fn merge_into(&mut self, offset: u32) {
        // Value offsets are always > 0 (there's at least a key before the value)
        let offset = NonZeroU32::new(offset).expect("value offset cannot be 0");
        self.min_offset = Some(match self.min_offset {
            Some(current) => current.min(offset),
            None => offset,
        });
        self.count += 1;
    }

    #[inline]
    fn count(&self) -> usize {
        self.count as usize
    }

    #[inline]
    fn offsets(&self) -> Option<&[u32]> {
        None
    }

    #[inline]
    fn min_offset(&self) -> Option<NonZeroU32> {
        self.min_offset
    }
}

impl RepeatedStorage for Vec<u32> {
    #[inline]
    fn merge_into(&mut self, offset: u32) {
        self.push(offset);
    }

    #[inline]
    fn count(&self) -> usize {
        self.len()
    }

    #[inline]
    fn offsets(&self) -> Option<&[u32]> {
        Some(self.as_slice())
    }

    #[inline]
    fn min_offset(&self) -> Option<NonZeroU32> {
        self.first().and_then(|&v| NonZeroU32::new(v))
    }
}

#[cfg(feature = "smallvec")]
impl<const N: usize> RepeatedStorage for smallvec::SmallVec<[u32; N]> {
    #[inline]
    fn merge_into(&mut self, offset: u32) {
        self.push(offset);
    }

    #[inline]
    fn count(&self) -> usize {
        self.len()
    }

    #[inline]
    fn offsets(&self) -> Option<&[u32]> {
        Some(self.as_slice())
    }

    #[inline]
    fn min_offset(&self) -> Option<NonZeroU32> {
        self.first().and_then(|&v| NonZeroU32::new(v))
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
    fn test_repeated_basic_storage() {
        use crate::wire::decode_key;
        use bytes::Buf;

        let buf = build_test_message();
        let bytes_buf = bytes::Bytes::from(buf);

        // Simulate decoding with BasicStorage using ProtoDecode
        let mut repeated: Repeated<ProtoString, BasicStorage> =
            <Repeated<ProtoString, BasicStorage> as ProtoDecode>::init(bytes_buf.clone(), 2);
        let mut slice = &bytes_buf[..];

        while slice.has_remaining() {
            let (wire_type, tag) = decode_key(&mut slice).unwrap();
            // Value offset is now (after key decode)
            let value_offset = bytes_buf.len() - slice.len();

            if tag == 2 {
                // Use decode_into - it records offset and skips the field
                <Repeated<ProtoString, BasicStorage> as ProtoDecode>::decode_into(
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
    }

    #[test]
    fn test_repeated_with_offsets() {
        use crate::wire::decode_key;
        use bytes::Buf;

        let buf = build_test_message();
        let bytes_buf = bytes::Bytes::from(buf);

        // Simulate decoding with offset storage using ProtoDecode
        let mut repeated: Repeated<ProtoString, Vec<u32>> =
            <Repeated<ProtoString, Vec<u32>> as ProtoDecode>::init(bytes_buf.clone(), 2);
        let mut slice = &bytes_buf[..];

        while slice.has_remaining() {
            let (wire_type, tag) = decode_key(&mut slice).unwrap();
            // Value offset is now (after key decode)
            let value_offset = bytes_buf.len() - slice.len();

            if tag == 2 {
                // Use decode_into - it records offset and skips the field
                <Repeated<ProtoString, Vec<u32>> as ProtoDecode>::decode_into(
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

        // Check ExactSizeIterator
        let iter = repeated.iter();
        assert_eq!(iter.len(), 3);

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
        let mut repeated: Repeated<ProtoString, BasicStorage> =
            <Repeated<ProtoString, BasicStorage> as ProtoDecode>::init(bytes_buf.clone(), 2);
        let mut slice = &bytes_buf[..];

        while slice.has_remaining() {
            let (wire_type, tag) = decode_key(&mut slice).unwrap();
            let value_offset = bytes_buf.len() - slice.len();

            if tag == 2 {
                <Repeated<ProtoString, BasicStorage> as ProtoDecode>::decode_into(
                    &mut slice,
                    &mut repeated,
                    value_offset,
                )
                .unwrap();
            } else {
                crate::wire::skip_field(wire_type, &mut slice).unwrap();
            }
        }

        // Now encode it back
        let mut encoded = Vec::new();
        repeated.encode(&mut encoded);

        // Verify encoded_len matches
        assert_eq!(encoded.len(), repeated.encoded_len());

        // Decode the encoded buffer and verify we get the same strings
        let encoded_bytes = bytes::Bytes::from(encoded);
        let mut decoded_strings = Vec::new();
        let mut slice = &encoded_bytes[..];

        while slice.has_remaining() {
            let (wire_type, tag) = decode_key(&mut slice).unwrap();
            assert_eq!(tag, 2);
            assert_eq!(wire_type, WireType::Len);

            let mut s = ProtoString::default();
            ProtoString::decode_into(&mut slice, &mut s, 0).unwrap();
            decoded_strings.push(s.as_str().to_string());
        }

        assert_eq!(decoded_strings, vec!["hello", "world", "!"]);
    }

    #[test]
    fn test_repeated_encode_empty() {
        let repeated: Repeated<ProtoString, BasicStorage> =
            <Repeated<ProtoString, BasicStorage> as ProtoDecode>::init(bytes::Bytes::new(), 1);

        assert_eq!(repeated.encoded_len(), 0);

        let mut encoded = Vec::new();
        repeated.encode(&mut encoded);
        assert!(encoded.is_empty());
    }
}
