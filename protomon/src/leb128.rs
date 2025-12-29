//! LEB128 variable-length integer encoding/decoding.

// This module uses `as` casts which have been thoroughly reviewed for correctness.
#![allow(clippy::as_conversions)]

use crate::error::DecodeErrorKind;

/// Types that can be decoded from a LEB128 encoded integer.
pub trait LebCodec: Sized {
    const MAX_LEB_BYTES: u32;

    /// Decode a LEB128 variable length integer from the provided pointer.
    ///
    /// Returns a tuple of the decoded value and the number of bytes read to
    /// decode said value.
    ///
    /// # Validating Result
    ///
    /// The user must check that the number of bytes used to decode the value
    /// is less than or equal to [`LebCodec::MAX_LEB_BYTES`]. See
    /// [`LebCodec::try_decode_leb128_safe`] for a version of this
    /// function that includes error handling.
    ///
    /// # Safety
    ///
    /// * __ValidRead__: The caller must ensure that `data` is valid for
    ///   `Self::MAX_LEB_BYTES` bytes to be read from the slice or that the last
    ///   byte is < `0x80` which signifies the end of
    ///
    /// See [`LebCodec::decode_leb128_safe`] for a version of this function
    /// that ensures the above safety variant.
    ///
    unsafe fn decode_leb128(data: &[u8]) -> Result<(Self, usize), DecodeErrorKind>;

    /// Decode a LEB128 variable.
    fn decode_leb128_safe(data: &[u8]) -> Result<(Self, usize), DecodeErrorKind> {
        // Fast path: if the input is long enough, we can decode directly.
        if data.len() >= Self::MAX_LEB_BYTES as usize {
            return unsafe { Self::decode_leb128(data) };
        }

        // Slow path: copy to a buffer that we know is safe to read from.
        let mut buffer = [0u8; 16];
        let len = data.len().min(16);
        buffer[..len].copy_from_slice(&data[..len]);

        unsafe { Self::decode_leb128(&buffer[..]) }
    }

    fn decode_leb128_buf<B: bytes::Buf>(buf: &mut B) -> Result<(Self, usize), DecodeErrorKind> {
        let chunk = buf.chunk();

        // Fast path: current chunk has enough bytes for direct decode.
        if chunk.len() >= Self::MAX_LEB_BYTES as usize {
            let (value, bytes_read) = unsafe { Self::decode_leb128(chunk)? };
            buf.advance(bytes_read);
            return Ok((value, bytes_read));
        }

        // Slow path: read byte by byte.
        let mut buffer = [0u8; 16];
        for i in 0..Self::MAX_LEB_BYTES as usize {
            if !buf.has_remaining() {
                return Err(DecodeErrorKind::invalid_varint());
            }
            buffer[i] = buf.get_u8();
            if buffer[i] < 0x80 {
                return unsafe { Self::decode_leb128(&buffer[..]) };
            }
        }
        Err(DecodeErrorKind::invalid_varint())
    }

    /// Encode `self` as a LEB128 variable length integer into the provided
    /// buffer.
    fn encode_leb128<B: bytes::BufMut>(self, buf: &mut B) -> usize;

    /// The number of bytes required to encode this integer.
    fn encoded_leb128_len(self) -> usize;
}

impl LebCodec for u64 {
    const MAX_LEB_BYTES: u32 = 10;

    #[inline(always)]
    unsafe fn decode_leb128(data: &[u8]) -> Result<(Self, usize), DecodeErrorKind> {
        // Byte 1.
        let mut b: u8 = unsafe { *data.get_unchecked(0) };
        let mut value = b as u64;
        if b < 0x80 {
            return Ok((value, 1));
        };
        value -= 0x80;

        // Byte 2.
        b = unsafe { *data.get_unchecked(1) };
        value += (b as u64) << 7;
        if b < 0x80 {
            return Ok((value, 2));
        };
        value -= 0x80 << 7;

        // Byte 3.
        b = unsafe { *data.get_unchecked(2) };
        value += (b as u64) << 14;
        if b < 0x80 {
            return Ok((value, 3));
        };
        value -= 0x80 << 14;

        // Byte 4.
        b = unsafe { *data.get_unchecked(3) };
        value += (b as u64) << 21;
        if b < 0x80 {
            return Ok((value, 4));
        };
        value -= 0x80 << 21;

        // Byte 5.
        b = unsafe { *data.get_unchecked(4) };
        value += (b as u64) << 28;
        if b < 0x80 {
            return Ok((value, 5));
        };
        value -= 0x80 << 28;

        // Byte 6.
        b = unsafe { *data.get_unchecked(5) };
        value += (b as u64) << 35;
        if b < 0x80 {
            return Ok((value, 6));
        };
        value -= 0x80 << 35;

        // Byte 7.
        b = unsafe { *data.get_unchecked(6) };
        value += (b as u64) << 42;
        if b < 0x80 {
            return Ok((value, 7));
        };
        value -= 0x80 << 42;

        // Byte 8.
        b = unsafe { *data.get_unchecked(7) };
        value += (b as u64) << 49;
        if b < 0x80 {
            return Ok((value, 8));
        };
        value -= 0x80 << 49;

        // Byte 9.
        b = unsafe { *data.get_unchecked(8) };
        value += (b as u64) << 56;
        if b < 0x80 {
            return Ok((value, 9));
        };
        value -= 0x80 << 56;

        // Byte 10.
        b = unsafe { *data.get_unchecked(9) };
        value += (b as u64) << 63;
        if b < 0x02 {
            return Ok((value, 10));
        };

        // Uh oh! We've read 10 bytes and either didn't find the final byte or
        // we overflowed u64::MAX.
        Err(DecodeErrorKind::invalid_varint())
    }

    #[inline]
    fn encode_leb128<B: bytes::BufMut>(self, buf: &mut B) -> usize {
        let mut value = self;

        // Byte 1.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 1;
        }
        buf.put_u8(byte | 0x80);

        // Byte 2.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 2;
        }
        buf.put_u8(byte | 0x80);

        // Byte 3.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 3;
        }
        buf.put_u8(byte | 0x80);

        // Byte 4.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 4;
        }
        buf.put_u8(byte | 0x80);

        // Byte 5.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 5;
        }
        buf.put_u8(byte | 0x80);

        // Byte 6.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 6;
        }
        buf.put_u8(byte | 0x80);

        // Byte 7.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 7;
        }
        buf.put_u8(byte | 0x80);

        // Byte 8.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 8;
        }
        buf.put_u8(byte | 0x80);

        // Byte 9.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 9;
        }
        buf.put_u8(byte | 0x80);

        // Byte 10.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        assert_eq!(value, 0);
        buf.put_u8(byte);
        10
    }

    /// Compute the LEB128 encoded length using leading_zeros.
    ///
    /// LEB128 encodes 7 bits per byte. The number of bytes needed is
    /// ceil(significant_bits / 7), with a minimum of 1 byte for value 0.
    ///
    /// For u64: significant_bits = 64 - leading_zeros
    /// bytes = ceil((64 - lz) / 7) = (64 - lz + 6) / 7 = (70 - lz) / 7
    ///
    /// The lookup table approach is faster than division because:
    /// 1. leading_zeros() compiles to a single LZCNT instruction (~1-3 cycles)
    /// 2. Table lookup is a single memory access (likely in L1 cache)
    /// 3. No division or branching required
    #[inline]
    fn encoded_leb128_len(self) -> usize {
        // Lookup table mapping leading_zeros (0-64) to LEB128 byte count.
        // Index 64 (value 0) maps to 1 byte.
        // Index 0 (all 64 bits used) maps to 10 bytes.
        //
        // For each lz value, significant_bits = 64 - lz
        // bytes = ceil(significant_bits / 7), minimum 1
        #[rustfmt::skip]
        const LZ_TO_LEN: [u8; 65] = [
            10,                                         // 0:     64 bits -> 10 bytes
            9, 9, 9, 9, 9, 9, 9,                        // 1-7:   63-57 bits -> 9 bytes
            8, 8, 8, 8, 8, 8, 8,                        // 8-14:  56-50 bits -> 8 bytes
            7, 7, 7, 7, 7, 7, 7,                        // 15-21: 49-43 bits -> 7 bytes
            6, 6, 6, 6, 6, 6, 6,                        // 22-28: 42-36 bits -> 6 bytes
            5, 5, 5, 5, 5, 5, 5,                        // 29-35: 35-29 bits -> 5 bytes
            4, 4, 4, 4, 4, 4, 4,                        // 36-42: 28-22 bits -> 4 bytes
            3, 3, 3, 3, 3, 3, 3,                        // 43-49: 21-15 bits -> 3 bytes
            2, 2, 2, 2, 2, 2, 2,                        // 50-56: 14-8 bits  -> 2 bytes
            1, 1, 1, 1, 1, 1, 1, 1,                     // 57-64: 7-0 bits   -> 1 byte
        ];

        // SAFETY: leading_zeros() returns 0-64 for u64, which is in bounds.
        LZ_TO_LEN[self.leading_zeros() as usize] as usize
    }
}

impl LebCodec for u32 {
    const MAX_LEB_BYTES: u32 = 5;

    #[inline]
    unsafe fn decode_leb128(data: &[u8]) -> Result<(Self, usize), DecodeErrorKind> {
        // Byte 1.
        let mut b: u8 = unsafe { *data.get_unchecked(0) };
        let mut value = b as u32;
        if b < 0x80 {
            return Ok((value, 1));
        };
        value -= 0x80;

        // Byte 2.
        b = unsafe { *data.get_unchecked(1) };
        value += (b as u32) << 7;
        if b < 0x80 {
            return Ok((value, 2));
        };
        value -= 0x80 << 7;

        // Byte 3.
        b = unsafe { *data.get_unchecked(2) };
        value += (b as u32) << 14;
        if b < 0x80 {
            return Ok((value, 3));
        };
        value -= 0x80 << 14;

        // Byte 4.
        b = unsafe { *data.get_unchecked(3) };
        value += (b as u32) << 21;
        if b < 0x80 {
            return Ok((value, 4));
        };
        value -= 0x80 << 21;

        // Byte 5.
        b = unsafe { *data.get_unchecked(4) };
        value += (b as u32) << 28;
        if b < 0x20 {
            return Ok((value, 5));
        };

        // Uh oh! We've read 5 bytes and either didn't find the final byte or
        // we overflowed u32::MAX.
        Err(DecodeErrorKind::invalid_varint())
    }

    #[inline]
    fn encode_leb128<B: bytes::BufMut>(self, buf: &mut B) -> usize {
        let mut value = self;

        // Byte 1.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 1;
        }
        buf.put_u8(byte | 0x80);

        // Byte 2.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 2;
        }
        buf.put_u8(byte | 0x80);

        // Byte 3.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 3;
        }
        buf.put_u8(byte | 0x80);

        // Byte 4.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            return 4;
        }
        buf.put_u8(byte | 0x80);

        // Byte 5.
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        assert_eq!(value, 0);
        buf.put_u8(byte);
        5
    }

    /// Compute the LEB128 encoded length using leading_zeros.
    ///
    /// For u32: significant_bits = 32 - leading_zeros
    /// bytes = ceil((32 - lz) / 7) = (32 - lz + 6) / 7 = (38 - lz) / 7
    #[inline]
    fn encoded_leb128_len(self) -> usize {
        // Lookup table mapping leading_zeros (0-32) to LEB128 byte count.
        #[rustfmt::skip]
        const LZ_TO_LEN: [u8; 33] = [
            5, 5, 5, 5,                         // 0-3:   32-29 bits -> 5 bytes
            4, 4, 4, 4, 4, 4, 4,                // 4-10:  28-22 bits -> 4 bytes
            3, 3, 3, 3, 3, 3, 3,                // 11-17: 21-15 bits -> 3 bytes
            2, 2, 2, 2, 2, 2, 2,                // 18-24: 14-8 bits  -> 2 bytes
            1, 1, 1, 1, 1, 1, 1, 1,             // 25-32: 7-0 bits   -> 1 byte
        ];

        // SAFETY: leading_zeros() returns 0-32 for u32, which is in bounds.
        LZ_TO_LEN[self.leading_zeros() as usize] as usize
    }
}

/// Branchless implementation for decoding a `u64` encoded with LEB128.
///
/// While theoretically this could be pretty, fast micro-benchmarks show this
/// actually performs worse than more naive implementations that decode a byte
/// at a time.
///
/// The implementation is left here to document optimization attempts.
///
/// ### TODO(parker):
///
/// * Benchmark on x86_64. Better performance of `LZCNT` (trailing zeros) and
///   `PEXT` (final combination of bits) could lead to overall better performance.
/// * Add a "b" impl which decodes one u64 and optionally returns early, then
///   decodes the second chunk only if necessary.
///
/// # Safety
///
/// The caller must guarantee that `data` is valid for a 16-byte read.
#[allow(dead_code)]
#[inline]
pub unsafe fn decode_u64_impl_a(data: *const u8) -> (u64, u32) {
    // Load 16 bytes at once as u128 to ensure a single read.
    //
    // SAFETY: Caller guarantees valid read.
    let full_value = unsafe { (data as *const u128).read_unaligned() };

    // Split into two u64s.
    let value_a = full_value as u64;
    let value_b = (full_value >> 64) as u64;

    // Determine how many bytes are part of the LEB128 integer.
    //
    // 1. Apply a mask to identify bytes where the most significant bit is
    //    zero.
    // 2. The bytes whose most significant bit was 1 are now all zeros. This
    //    means we can get our answer by counting the trailing zeros and
    //    dividing by 8 (aka bit shifting right by 3).
    let not_msb_a = !value_a & 0x80808080_80808080u64;
    let not_msb_b = !value_b & 0x80808080_80808080u64;

    let leb_num_bytes_a = not_msb_a.trailing_zeros() >> 3;

    // Mask away the bits that are not needed for the integer.
    //
    // 1. Subtract 1 from `not_msb` will invert all of the 0 bits before the
    //    first 1 bit.
    // 2. XORing against itself will turn all of the following 1 bits to 0.
    let mask_a = not_msb_a.wrapping_sub(1) ^ not_msb_a;
    // 3. Zero out the second mask if the number is entirely contained
    //    within the first word.
    let mask_b = (not_msb_b.wrapping_sub(1) ^ not_msb_b) * (not_msb_a == 0) as u64;
    // 4. ANDing with the original value results in `leb_part` being only the
    //    bits that are part of the LEB128 encoded integer.
    let leb_part_a = mask_a & value_a;
    let leb_part_b = mask_b & value_b;

    // Merge all of the LEB128 sections together.

    let final_value = {
        #[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
        {
            // On x86_64 architectures with the BMI2 instruciton set we can do
            // the entire merge with just a single instruction that generally
            // takes ~3 cycles.
            //
            // TODO(parker): 'pext' is relatively slow on AMD Zen architectures
            // before Zen 5, for those we should fallback to the more naive
            // implementation.
            let part_a = core::arch::x86_64::_pext_u64(leb_part_a, 0x7f7f7f7f7f7f7f7f);
            let part_b = core::arch::x86_64::_pext_u64(leb_part_b, 0x000000000000017F);

            // Splice the two parts together.
            part_a | (part_b << 56)
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "bmi2")))]
        {
            // Note(parker): While we know our number is only `leb_num_bytes` long
            // it's more efficient to just process every chunk as opposed to branching
            // on the number of bytes.
            //
            // The thinking here is that AND, OR, and SHIFT take about 1 cycle each on
            // a modern CPU and usually multiple can be done in parallel, so ~1 cycle
            // per chunk here. Meanwhile a mispredicted branch takes at the very least
            // 10 cycles, so it's just cheaper to rip through all the chunks.
            (leb_part_a & 0x000000000000007f)
                | ((leb_part_a & 0x0000000000007f00) >> 1)
                | ((leb_part_a & 0x00000000007f0000) >> 2)
                | ((leb_part_a & 0x000000007f000000) >> 3)
                | ((leb_part_a & 0x0000007f00000000) >> 4)
                | ((leb_part_a & 0x00007f0000000000) >> 5)
                | ((leb_part_a & 0x007f000000000000) >> 6)
                | ((leb_part_a & 0x7f00000000000000) >> 7)
                | ((leb_part_b & 0x000000000000007f) << 56)
                | ((leb_part_b & 0x0000000000000100) << 55)
        }
    };

    // This if-statement compiles to CMOV on x86_64.
    //
    // Note: Previously we used the following branchless implementation:
    // ```
    // let leb_num_bytes = (leb_num_bytes_a + leb_num_bytes_b & !len_a_only_mask)
    //  | (leb_num_bytes_a & len_a_only_mask);
    // ```
    // But this branch performs better.
    let leb_num_bytes = if not_msb_a == 0 {
        leb_num_bytes_a + (not_msb_b.trailing_zeros() >> 3)
    } else {
        leb_num_bytes_a
    };

    (final_value, leb_num_bytes + 1)
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use proptest::prelude::*;
    use proptest::property_test;

    use crate::leb128::decode_u64_impl_a;

    use super::LebCodec;

    #[test]
    fn smoketest_leb128_decode_u64() {
        #[track_caller]
        fn test_case(val: u64, len: usize) {
            let mut buffer: [u8; 16] = [0u8; 16];
            let encode_len = u64::encode_leb128(val, &mut buffer.as_mut_slice());

            // SAFETY - ValidRead: We created a buffer of 16 bytes.
            let (rnd, rnd_len) = unsafe { u64::decode_leb128(&buffer[..]).unwrap() };

            assert_eq!(rnd, val, "invalid value");
            assert_eq!(len, rnd_len, "invalid length");
            assert_eq!(len, encode_len, "invalid encode length");
        }

        test_case(0, 1);
        test_case(1, 1);
        test_case(42, 1);
        test_case(128, 2);
        // First case that spills to multiple words.
        test_case(72057594037927937, 9);
        test_case(u64::MAX, 10);
    }

    #[test]
    fn smoketest_leb128_decode_u32() {
        #[track_caller]
        fn test_case(val: u32, len: usize) {
            let mut buffer: [u8; 8] = [0u8; 8];
            let encode_len = u32::encode_leb128(val, &mut buffer.as_mut_slice());

            // SAFETY - ValidRead: We created a buffer of 16 bytes.
            let (rnd, rnd_len) = unsafe { u32::decode_leb128(&buffer[..]).unwrap() };

            assert_eq!(rnd, val, "invalid value");
            assert_eq!(len, rnd_len, "invalid length");
            assert_eq!(len, encode_len, "invalid encode length");
        }

        test_case(0, 1);
        test_case(1, 1);
        test_case(42, 1);
        test_case(128, 2);
        test_case(u32::MAX, 5);
    }

    #[property_test]
    fn proptest_leb128_u64(val: u64) {
        let mut buffer: [u8; 16] = [0u8; 16];
        let og_len = u64::encode_leb128(val, &mut buffer.as_mut_slice());

        // SAFETY - ValidRead: We created a buffer of 16 bytes.
        let (rnd, len) = unsafe { u64::decode_leb128(&buffer[..]).unwrap() };
        prop_assert_eq!(rnd, val, "invalid value");
        prop_assert_eq!(len, og_len, "invalid length");

        let encoded_len = rnd.encoded_leb128_len();
        prop_assert_eq!(encoded_len, len);
    }

    #[property_test]
    fn proptest_leb128_decode_u32(val: u32) {
        let mut buffer: [u8; 8] = [0u8; 8];
        let og_len = u32::encode_leb128(val, &mut buffer.as_mut_slice());

        // SAFETY - ValidRead: We created a buffer of 16 bytes.
        let (rnd, len) = unsafe { u32::decode_leb128(&buffer[..]).unwrap() };
        prop_assert_eq!(rnd, val);
        prop_assert_eq!(len, og_len);

        let encoded_len = rnd.encoded_leb128_len();
        prop_assert_eq!(encoded_len, len);
    }

    #[property_test]
    fn proptest_leb128_decode_u64_a(val: u64) {
        let mut buffer: [u8; 16] = [0u8; 16];
        let og_len = val.encode_leb128(&mut buffer.as_mut_slice());

        // Note: Despite being deadcode we continue to test decode_u64_impl_a
        // to ensure its implementation remains correct.

        // SAFETY - ValidRead: We created a buffer of 16 bytes.
        let (rnd, len) = unsafe { decode_u64_impl_a(buffer.as_ptr()) };
        prop_assert_eq!(rnd, val);
        prop_assert_eq!(len, og_len as u32);
    }
}
