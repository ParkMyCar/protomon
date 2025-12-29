//! Packed repeated field types and optimized decoding.
//!
//! This module provides:
//! - `ProtoPacked<T>`: Zero-copy packed field storage with lazy/batch decoding
//! - `ProtoPackedIter<T>`: Lazy iterator over packed values
//! - `PackedDecode`: Trait for optimized batch decoding

use bytes::Bytes;
use core::marker::PhantomData;

use crate::codec::{
    Fixed32, Fixed64, ProtoDecode, ProtoEncode, ProtoType, Sfixed32, Sfixed64, Sint32, Sint64,
};
use crate::error::DecodeErrorKind;
use crate::leb128::LebCodec;
use crate::util::{CastFrom, TruncatingCastFrom};
use crate::wire::WireType;
use alloc::vec::Vec;

#[cfg(feature = "smallvec")]
use smallvec::SmallVec;

#[cfg(feature = "smallvec")]
type ChunkVec = SmallVec<[Bytes; 1]>;

#[cfg(not(feature = "smallvec"))]
type ChunkVec = Vec<Bytes>;

/// Zero-copy packed repeated field.
///
/// Stores raw bytes from the wire format and decodes on demand. Multiple
/// chunks are supported because packed fields can appear multiple times
/// in a message (the values are concatenated).
///
/// # Example
///
/// ```ignore
/// // Lazy iteration (decodes one element at a time)
/// for value in packed.iter() {
///     println!("{}", value?);
/// }
///
/// // Fast batch decoding
/// let values: Vec<u32> = packed.decode()?;
///
/// // Direct bytes access (for Arrow, etc.)
/// for chunk in packed.chunks() {
///     // chunk is &Bytes, can be zero-copy sliced
/// }
/// ```
pub struct ProtoPacked<T> {
    chunks: ChunkVec,
    _marker: PhantomData<T>,
}

impl<T> Default for ProtoPacked<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for ProtoPacked<T> {
    fn clone(&self) -> Self {
        Self {
            chunks: self.chunks.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> core::fmt::Debug for ProtoPacked<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProtoPacked")
            .field("chunks", &self.chunks.len())
            .field("bytes", &self.byte_len())
            .finish()
    }
}

impl<T> ProtoPacked<T> {
    /// Create an empty packed field.
    #[inline]
    pub fn new() -> Self {
        Self {
            chunks: ChunkVec::new(),
            _marker: PhantomData,
        }
    }

    /// Create from a single chunk of bytes.
    #[inline]
    pub fn from_bytes(bytes: Bytes) -> Self {
        let mut chunks = ChunkVec::new();
        if !bytes.is_empty() {
            chunks.push(bytes);
        }
        Self {
            chunks,
            _marker: PhantomData,
        }
    }

    /// Add a chunk of packed data (called during decoding).
    #[inline]
    pub fn push_chunk(&mut self, chunk: Bytes) {
        if !chunk.is_empty() {
            self.chunks.push(chunk);
        }
    }

    /// Get the raw byte chunks (for Arrow interop, etc.).
    #[inline]
    pub fn chunks(&self) -> &[Bytes] {
        &self.chunks
    }

    /// Total byte length across all chunks.
    #[inline]
    pub fn byte_len(&self) -> usize {
        self.chunks.iter().map(|c| c.len()).sum()
    }

    /// Returns true if there are no bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.chunks.iter().all(|c| c.is_empty())
    }

    /// Number of chunks (usually 1, but can be more if field appeared multiple times).
    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

impl<T: ProtoDecode + Default> ProtoPacked<T> {
    /// Lazy iterator over all values across all chunks.
    #[inline]
    pub fn iter(&self) -> ProtoPackedIter<'_, T> {
        ProtoPackedIter {
            chunks: &self.chunks,
            chunk_idx: 0,
            offset: 0,
            _marker: PhantomData,
        }
    }
}

impl<T: PackedDecode> ProtoPacked<T> {
    /// Decode all values into the provided Vec (fastest method).
    #[inline]
    pub fn decode_into(&self, dst: &mut Vec<T>) -> Result<(), DecodeErrorKind> {
        for chunk in &self.chunks {
            T::decode_packed_into(chunk, dst)?;
        }
        Ok(())
    }

    /// Decode all values into a new Vec.
    #[inline]
    pub fn decode(&self) -> Result<Vec<T>, DecodeErrorKind> {
        let mut result = Vec::with_capacity(self.byte_len() / 2);
        self.decode_into(&mut result)?;
        Ok(result)
    }
}

impl<T: ProtoType> ProtoType for ProtoPacked<T> {
    const WIRE_TYPE: WireType = WireType::Len;
}

impl<T: ProtoType> ProtoDecode for ProtoPacked<T> {
    /// Decode a packed chunk and add it to this field.
    ///
    /// Each occurrence of the field tag adds another chunk of packed data.
    #[inline]
    fn decode_into<B: bytes::Buf>(
        buf: &mut B,
        dst: &mut Self,
        _offset: usize,
    ) -> Result<(), DecodeErrorKind> {
        use bytes::Buf;
        let len = crate::wire::decode_len(buf)?;
        if buf.remaining() < len {
            return Err(DecodeErrorKind::UnexpectedEndOfBuffer);
        }
        let chunk = buf.copy_to_bytes(len);
        dst.push_chunk(chunk);
        Ok(())
    }
}

impl<T: ProtoType + ProtoEncode + PackedDecode + 'static> super::ProtoRepeated for ProtoPacked<T> {
    /// No initialization needed for ProtoPacked.
    #[inline]
    fn init_repeated(&mut self, _msg_buf: &bytes::Bytes, _tag: u32) {
        // ProtoPacked doesn't need buffer context - it stores its own chunks
    }

    /// Encode all elements as a single packed field.
    #[inline]
    fn encode_repeated<B: bytes::BufMut>(&self, tag: u32, buf: &mut B) {
        if self.is_empty() {
            return;
        }
        crate::wire::encode_key(WireType::Len, tag, buf);
        self.encode(buf);
    }

    #[inline]
    fn encoded_repeated_len(&self, tag: u32) -> usize {
        if self.is_empty() {
            return 0;
        }
        crate::wire::encoded_key_len(tag) + self.encoded_len()
    }

    #[inline]
    fn repeated_len(&self) -> usize {
        // For packed fields, we don't know the count without decoding.
        // This is mainly used for is_empty checks.
        if self.is_empty() {
            0
        } else {
            1 // Non-zero indicates not empty
        }
    }

    #[inline]
    fn is_repeated_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<T: ProtoEncode> ProtoEncode for ProtoPacked<T> {
    fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
        let total_len = u64::cast_from(self.byte_len());
        total_len.encode_leb128(buf);
        for chunk in &self.chunks {
            buf.put_slice(chunk);
        }
    }

    fn encoded_len(&self) -> usize {
        let total_len = u64::cast_from(self.byte_len());
        total_len.encoded_leb128_len() + self.byte_len()
    }
}

/// Lazy iterator over packed values across multiple chunks.
pub struct ProtoPackedIter<'a, T> {
    chunks: &'a [Bytes],
    chunk_idx: usize,
    offset: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: ProtoDecode + Default> Iterator for ProtoPackedIter<'a, T> {
    type Item = Result<T, DecodeErrorKind>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.chunk_idx < self.chunks.len() {
            let chunk = &self.chunks[self.chunk_idx];
            if self.offset >= chunk.len() {
                self.chunk_idx += 1;
                self.offset = 0;
                continue;
            }

            let mut slice = &chunk[self.offset..];
            let start_len = slice.len();
            let mut value = T::default();

            match T::decode_into(&mut slice, &mut value, self.offset) {
                Ok(()) => {
                    self.offset += start_len - slice.len();
                    return Some(Ok(value));
                }
                Err(e) => return Some(Err(e)),
            }
        }
        None
    }
}

/// Trait for types that can be batch-decoded from packed format.
pub trait PackedDecode: Sized + Default + Clone {
    /// Decode all packed elements into a vector.
    fn decode_packed_into(data: &[u8], dst: &mut Vec<Self>) -> Result<(), DecodeErrorKind>;

    /// Decode all packed elements and return them as a new vector.
    fn decode_packed(data: &[u8]) -> Result<Vec<Self>, DecodeErrorKind>;
}

trait PackedElement: Sized + Copy {
    unsafe fn read_le(ptr: *const u8) -> Self;
}

macro_rules! impl_packed_fixed_4byte {
    ($($ty:ty => $read:expr),+ $(,)?) => {$(
        impl PackedElement for $ty {
            #[allow(clippy::as_conversions)]
            #[inline(always)]
            unsafe fn read_le(ptr: *const u8) -> Self { $read(ptr) }
        }

        impl PackedDecode for $ty {
            #[inline]
            fn decode_packed_into(data: &[u8], dst: &mut Vec<Self>) -> Result<(), DecodeErrorKind> {
                decode_packed_4byte(data, dst)
            }

            #[inline]
            fn decode_packed(data: &[u8]) -> Result<Vec<Self>, DecodeErrorKind> {
                let mut result = Vec::with_capacity(data.len() / 4);
                decode_packed_4byte(data, &mut result)?;
                Ok(result)
            }
        }
    )+};
}

macro_rules! impl_packed_fixed_8byte {
    ($($ty:ty => $read:expr),+ $(,)?) => {$(
        impl PackedElement for $ty {
            #[allow(clippy::as_conversions)]
            #[inline(always)]
            unsafe fn read_le(ptr: *const u8) -> Self { $read(ptr) }
        }

        impl PackedDecode for $ty {
            #[inline]
            fn decode_packed_into(data: &[u8], dst: &mut Vec<Self>) -> Result<(), DecodeErrorKind> {
                decode_packed_8byte(data, dst)
            }

            #[inline]
            fn decode_packed(data: &[u8]) -> Result<Vec<Self>, DecodeErrorKind> {
                let mut result = Vec::with_capacity(data.len() / 8);
                decode_packed_8byte(data, &mut result)?;
                Ok(result)
            }
        }
    )+};
}

impl_packed_fixed_4byte! {
    Fixed32 => |ptr| Fixed32(read_u32_le(ptr)),
    Sfixed32 => |ptr| Sfixed32(read_u32_le(ptr) as i32),
    f32 => |ptr| f32::from_bits(read_u32_le(ptr)),
}

impl_packed_fixed_8byte! {
    Fixed64 => |ptr| Fixed64(read_u64_le(ptr)),
    Sfixed64 => |ptr| Sfixed64(read_u64_le(ptr) as i64),
    f64 => |ptr| f64::from_bits(read_u64_le(ptr)),
}

#[inline]
fn decode_packed_4byte<T: PackedElement>(
    data: &[u8],
    dst: &mut Vec<T>,
) -> Result<(), DecodeErrorKind> {
    let len = data.len();
    if len % 4 != 0 {
        return Err(DecodeErrorKind::InvalidPackedLength {
            expected_multiple: 4,
            actual: u32::truncating_cast_from(len),
        });
    }

    let count = len / 4;
    dst.reserve(count);

    let mut ptr = data.as_ptr();
    let chunks = len / 16;

    for _ in 0..chunks {
        unsafe {
            dst.push(T::read_le(ptr));
            dst.push(T::read_le(ptr.add(4)));
            dst.push(T::read_le(ptr.add(8)));
            dst.push(T::read_le(ptr.add(12)));
            ptr = ptr.add(16);
        }
    }

    for _ in 0..(count - chunks * 4) {
        unsafe {
            dst.push(T::read_le(ptr));
            ptr = ptr.add(4);
        }
    }

    Ok(())
}

#[inline]
fn decode_packed_8byte<T: PackedElement>(
    data: &[u8],
    dst: &mut Vec<T>,
) -> Result<(), DecodeErrorKind> {
    let len = data.len();
    if len % 8 != 0 {
        return Err(DecodeErrorKind::InvalidPackedLength {
            expected_multiple: 8,
            actual: u32::truncating_cast_from(len),
        });
    }

    let count = len / 8;
    dst.reserve(count);

    let mut ptr = data.as_ptr();
    let chunks = len / 16;

    for _ in 0..chunks {
        unsafe {
            dst.push(T::read_le(ptr));
            dst.push(T::read_le(ptr.add(8)));
            ptr = ptr.add(16);
        }
    }

    if count > chunks * 2 {
        unsafe {
            dst.push(T::read_le(ptr));
        }
    }

    Ok(())
}

#[inline(always)]
#[allow(clippy::as_conversions)] // Pointer cast for unaligned read.
unsafe fn read_u32_le(ptr: *const u8) -> u32 {
    u32::from_le((ptr as *const u32).read_unaligned())
}

#[inline(always)]
#[allow(clippy::as_conversions)] // Pointer cast for unaligned read.
unsafe fn read_u64_le(ptr: *const u8) -> u64 {
    u64::from_le((ptr as *const u64).read_unaligned())
}

#[inline]
fn decode_packed_varint<T, L: LebCodec, F>(
    data: &[u8],
    dst: &mut Vec<T>,
    convert: F,
) -> Result<(), DecodeErrorKind>
where
    F: Fn(L) -> T,
{
    let mut offset = 0;
    let len = data.len();

    while offset + usize::cast_from(L::MAX_LEB_BYTES) <= len {
        let (value, bytes_read) = unsafe { L::decode_leb128(&data[offset..])? };
        dst.push(convert(value));
        offset += bytes_read;
    }

    while offset < len {
        let (value, bytes_read) = L::decode_leb128_safe(&data[offset..])?;
        dst.push(convert(value));
        offset += bytes_read;
    }
    Ok(())
}

macro_rules! impl_packed_varint {
    ($ty:ty, $leb:ty, $cap_div:expr, $convert:expr) => {
        impl PackedDecode for $ty {
            #[inline]
            #[allow(clippy::as_conversions)]
            fn decode_packed_into(data: &[u8], dst: &mut Vec<Self>) -> Result<(), DecodeErrorKind> {
                decode_packed_varint::<$ty, $leb, _>(data, dst, $convert)
            }

            #[inline]
            fn decode_packed(data: &[u8]) -> Result<Vec<Self>, DecodeErrorKind> {
                let mut result = Vec::with_capacity(data.len() / $cap_div);
                Self::decode_packed_into(data, &mut result)?;
                Ok(result)
            }
        }
    };
}

impl_packed_varint!(u32, u32, 2, |v| v);
impl_packed_varint!(u64, u64, 2, |v| v);
impl_packed_varint!(i32, u64, 2, |v: u64| v as i32);
impl_packed_varint!(i64, u64, 2, |v: u64| v as i64);
impl_packed_varint!(bool, u64, 1, |v: u64| v != 0);

#[allow(clippy::as_conversions)]
impl PackedDecode for Sint32 {
    #[inline]
    fn decode_packed_into(data: &[u8], dst: &mut Vec<Self>) -> Result<(), DecodeErrorKind> {
        decode_packed_varint::<Sint32, u32, _>(data, dst, |v| {
            Sint32(((v >> 1) as i32) ^ -((v & 1) as i32))
        })
    }

    #[inline]
    fn decode_packed(data: &[u8]) -> Result<Vec<Self>, DecodeErrorKind> {
        let mut result = Vec::with_capacity(data.len() / 2);
        Self::decode_packed_into(data, &mut result)?;
        Ok(result)
    }
}

#[allow(clippy::as_conversions)]
impl PackedDecode for Sint64 {
    #[inline]
    fn decode_packed_into(data: &[u8], dst: &mut Vec<Self>) -> Result<(), DecodeErrorKind> {
        decode_packed_varint::<Sint64, u64, _>(data, dst, |v| {
            Sint64(((v >> 1) as i64) ^ -((v & 1) as i64))
        })
    }

    #[inline]
    fn decode_packed(data: &[u8]) -> Result<Vec<Self>, DecodeErrorKind> {
        let mut result = Vec::with_capacity(data.len() / 2);
        Self::decode_packed_into(data, &mut result)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn encode_values<T: ProtoEncode>(values: &[T]) -> Vec<u8> {
        let mut buf = Vec::new();
        for v in values {
            v.encode(&mut buf);
        }
        buf
    }

    #[test]
    fn test_proto_packed_decode() {
        let values = vec![Fixed32(1), Fixed32(2), Fixed32(3)];
        let encoded = Bytes::from(encode_values(&values));

        let packed = ProtoPacked::<Fixed32>::from_bytes(encoded);
        assert_eq!(packed.decode().unwrap(), values);
    }

    #[test]
    fn test_proto_packed_iter() {
        let values = vec![Fixed32(10), Fixed32(20), Fixed32(30)];
        let encoded = Bytes::from(encode_values(&values));

        let packed = ProtoPacked::<Fixed32>::from_bytes(encoded);
        let decoded: Result<Vec<_>, _> = packed.iter().collect();
        assert_eq!(decoded.unwrap(), values);
    }

    #[test]
    fn test_proto_packed_multiple_chunks() {
        let chunk1 = Bytes::from(encode_values(&[Fixed32(1), Fixed32(2)]));
        let chunk2 = Bytes::from(encode_values(&[Fixed32(3), Fixed32(4)]));

        let mut packed = ProtoPacked::<Fixed32>::new();
        packed.push_chunk(chunk1);
        packed.push_chunk(chunk2);

        assert_eq!(packed.chunk_count(), 2);
        assert_eq!(
            packed.decode().unwrap(),
            vec![Fixed32(1), Fixed32(2), Fixed32(3), Fixed32(4)]
        );
    }

    #[test]
    fn test_proto_packed_empty() {
        let packed = ProtoPacked::<Fixed32>::new();
        assert!(packed.is_empty());
        assert_eq!(packed.decode().unwrap(), vec![]);
    }

    #[test]
    fn test_proto_packed_chunks_access() {
        let encoded = Bytes::from(encode_values(&[Fixed32(1), Fixed32(2)]));
        let packed = ProtoPacked::<Fixed32>::from_bytes(encoded.clone());
        assert_eq!(packed.chunks(), &[encoded]);
    }

    #[test]
    fn test_decode_packed_fixed32() {
        let values = vec![Fixed32(1), Fixed32(2), Fixed32(u32::MAX)];
        let encoded = encode_values(&values);
        assert_eq!(Fixed32::decode_packed(&encoded).unwrap(), values);
    }

    #[test]
    fn test_decode_packed_fixed64() {
        let values = vec![Fixed64(1), Fixed64(2), Fixed64(u64::MAX)];
        let encoded = encode_values(&values);
        assert_eq!(Fixed64::decode_packed(&encoded).unwrap(), values);
    }

    #[test]
    fn test_decode_packed_sfixed32() {
        let values = vec![Sfixed32(-1), Sfixed32(0), Sfixed32(i32::MAX)];
        let encoded = encode_values(&values);
        assert_eq!(Sfixed32::decode_packed(&encoded).unwrap(), values);
    }

    #[test]
    fn test_decode_packed_sfixed64() {
        let values = vec![Sfixed64(-1), Sfixed64(0), Sfixed64(i64::MAX)];
        let encoded = encode_values(&values);
        assert_eq!(Sfixed64::decode_packed(&encoded).unwrap(), values);
    }

    #[test]
    fn test_decode_packed_f32() {
        let values = vec![1.0f32, -2.5f32, f32::MAX];
        let encoded = encode_values(&values);
        assert_eq!(f32::decode_packed(&encoded).unwrap(), values);
    }

    #[test]
    fn test_decode_packed_f64() {
        let values = vec![1.0f64, -2.5f64, f64::MAX];
        let encoded = encode_values(&values);
        assert_eq!(f64::decode_packed(&encoded).unwrap(), values);
    }

    #[test]
    fn test_decode_packed_fixed32_large() {
        let values: Vec<Fixed32> = (0..1000).map(Fixed32).collect();
        let encoded = encode_values(&values);
        assert_eq!(Fixed32::decode_packed(&encoded).unwrap(), values);
    }

    #[test]
    fn test_decode_packed_fixed64_large() {
        let values: Vec<Fixed64> = (0..1000).map(Fixed64).collect();
        let encoded = encode_values(&values);
        assert_eq!(Fixed64::decode_packed(&encoded).unwrap(), values);
    }

    #[test]
    fn test_decode_packed_empty() {
        assert!(Fixed32::decode_packed(&[]).unwrap().is_empty());
    }

    #[test]
    fn test_decode_packed_invalid_length() {
        assert!(Fixed32::decode_packed(&[1, 2, 3]).is_err());
    }
}
