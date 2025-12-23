//! Optimized packed repeated field decoding.
//!
//! For fixed-size types, batched processing enables LLVM auto-vectorization.
//! For varints, direct pointer arithmetic minimizes per-element overhead.

use crate::codec::{Fixed32, Fixed64, Sfixed32, Sfixed64, Sint32, Sint64};
use crate::error::DecodeErrorKind;
use crate::leb128::LebCodec;
use alloc::vec::Vec;

/// Trait for types that can be batch-decoded from packed format.
pub trait PackedDecode: Sized + Default + Clone {
    /// Decode all packed elements into a vector.
    fn decode_packed_into(data: &[u8], dst: &mut Vec<Self>) -> Result<(), DecodeErrorKind>;

    /// Decode all packed elements and return them as a new vector.
    fn decode_packed(data: &[u8]) -> Result<Vec<Self>, DecodeErrorKind>;
}

/// Trait for fixed-size types that can be read from raw bytes.
trait PackedElement: Sized + Copy {
    unsafe fn read_le(ptr: *const u8) -> Self;
}

macro_rules! impl_packed_fixed_4byte {
    ($($ty:ty => $read:expr),+ $(,)?) => {$(
        impl PackedElement for $ty {
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

/// Decode packed 4-byte elements with loop structure that enables auto-vectorization.
#[inline]
fn decode_packed_4byte<T: PackedElement>(
    data: &[u8],
    dst: &mut Vec<T>,
) -> Result<(), DecodeErrorKind> {
    let len = data.len();
    if len % 4 != 0 {
        return Err(DecodeErrorKind::InvalidPackedLength {
            expected_multiple: 4,
            actual: len as u32,
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

/// Decode packed 8-byte elements with loop structure that enables auto-vectorization.
#[inline]
fn decode_packed_8byte<T: PackedElement>(
    data: &[u8],
    dst: &mut Vec<T>,
) -> Result<(), DecodeErrorKind> {
    let len = data.len();
    if len % 8 != 0 {
        return Err(DecodeErrorKind::InvalidPackedLength {
            expected_multiple: 8,
            actual: len as u32,
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
unsafe fn read_u32_le(ptr: *const u8) -> u32 {
    u32::from_le((ptr as *const u32).read_unaligned())
}

#[inline(always)]
unsafe fn read_u64_le(ptr: *const u8) -> u64 {
    u64::from_le((ptr as *const u64).read_unaligned())
}

/// Generic varint decoder with fast/slow path split.
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

    // Fast path: enough bytes for unsafe decode
    while offset + L::MAX_LEB_BYTES as usize <= len {
        let (value, bytes_read) = unsafe { L::decode_leb128(&data[offset..])? };
        dst.push(convert(value));
        offset += bytes_read;
    }

    // Slow path for final elements
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

// Sint32/Sint64 need zigzag decoding.
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
    use crate::codec::ProtoEncode;
    use alloc::vec;

    fn encode_values<T: ProtoEncode>(values: &[T]) -> Vec<u8> {
        let mut buf = Vec::new();
        for v in values {
            v.encode(&mut buf);
        }
        buf
    }

    #[test]
    fn test_decode_packed_fixed32() {
        let values = vec![Fixed32(1), Fixed32(2), Fixed32(u32::MAX)];
        let encoded = encode_values(&values);
        let decoded = Fixed32::decode_packed(&encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_decode_packed_fixed64() {
        let values = vec![Fixed64(1), Fixed64(2), Fixed64(u64::MAX)];
        let encoded = encode_values(&values);
        let decoded = Fixed64::decode_packed(&encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_decode_packed_sfixed32() {
        let values = vec![Sfixed32(-1), Sfixed32(0), Sfixed32(i32::MAX)];
        let encoded = encode_values(&values);
        let decoded = Sfixed32::decode_packed(&encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_decode_packed_sfixed64() {
        let values = vec![Sfixed64(-1), Sfixed64(0), Sfixed64(i64::MAX)];
        let encoded = encode_values(&values);
        let decoded = Sfixed64::decode_packed(&encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_decode_packed_f32() {
        let values = vec![1.0f32, -2.5f32, f32::MAX];
        let encoded = encode_values(&values);
        let decoded = f32::decode_packed(&encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_decode_packed_f64() {
        let values = vec![1.0f64, -2.5f64, f64::MAX];
        let encoded = encode_values(&values);
        let decoded = f64::decode_packed(&encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_decode_packed_fixed32_large() {
        let values: Vec<Fixed32> = (0..1000).map(Fixed32).collect();
        let encoded = encode_values(&values);
        let decoded = Fixed32::decode_packed(&encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_decode_packed_fixed64_large() {
        let values: Vec<Fixed64> = (0..1000).map(Fixed64).collect();
        let encoded = encode_values(&values);
        let decoded = Fixed64::decode_packed(&encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_decode_packed_empty() {
        let decoded = Fixed32::decode_packed(&[]).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_decode_packed_invalid_length() {
        let result = Fixed32::decode_packed(&[1, 2, 3]);
        assert!(result.is_err());
    }
}
