/// Types that can be decoded from a LEB128 encoded integer.
pub trait VarIntegerTarget: Sized {
    /// Decode a LEB128 variable length integer from the provided pointer.
    ///
    /// # Safety
    ///
    /// * __ValidRead__: The caller must ensure that `data` is valid for
    ///   `Self::NUM_U64S_READ * 8` bytes to be read from the pointer.
    ///
    /// See [`VarIntegerTarget::decode_slice`] for a version of this function
    /// that ensures the above safety variant.
    ///
    unsafe fn decode(data: &[u8]) -> (Self, u32);

    /// Decode a LEB128 variable
    fn decode_safe(data: &[u8]) -> (Self, u32) {
        // Copy all of the available bytes into a buffer that we know are safe
        // to read from.
        //
        // N.B. For all implementations 16 bytes is the most we'll ever need.
        let mut buffer = [0u8; 16];
        let len = data.len().min(16);
        buffer[..len].copy_from_slice(&data[..len]);

        unsafe { Self::decode(&buffer[..]) }
    }
}

impl VarIntegerTarget for u64 {
    #[inline]
    unsafe fn decode(data: &[u8]) -> (Self, u32) {
        // Byte 1.
        let mut b: u8 = unsafe { *data.get_unchecked(0) };
        let mut value = u64::from(b);
        if b < 0x80 {
            return (value, 1);
        };
        value -= 0x80;

        // Byte 2.
        b = unsafe { *data.get_unchecked(1) };
        value += u64::from(b) << 7;
        if b < 0x80 {
            return (value, 2);
        };
        value -= 0x80 << 7;

        // Byte 3.
        b = unsafe { *data.get_unchecked(2) };
        value += u64::from(b) << 14;
        if b < 0x80 {
            return (value, 3);
        };
        value -= 0x80 << 14;

        // Byte 4.
        b = unsafe { *data.get_unchecked(3) };
        value += u64::from(b) << 21;
        if b < 0x80 {
            return (value, 4);
        };
        value -= 0x80 << 21;

        // Byte 5.
        b = unsafe { *data.get_unchecked(4) };
        value += u64::from(b) << 28;
        if b < 0x80 {
            return (value, 5);
        };
        value -= 0x80 << 28;

        // Byte 6.
        b = unsafe { *data.get_unchecked(5) };
        value += u64::from(b) << 35;
        if b < 0x80 {
            return (value, 6);
        };
        value -= 0x80 << 35;

        // Byte 7.
        b = unsafe { *data.get_unchecked(6) };
        value += u64::from(b) << 42;
        if b < 0x80 {
            return (value, 7);
        };
        value -= 0x80 << 42;

        // Byte 8.
        b = unsafe { *data.get_unchecked(7) };
        value += u64::from(b) << 49;
        if b < 0x80 {
            return (value, 8);
        };
        value -= 0x80 << 49;

        // Byte 9.
        b = unsafe { *data.get_unchecked(8) };
        value += u64::from(b) << 56;
        if b < 0x80 {
            return (value, 9);
        };
        value -= 0x80 << 56;

        // Byte 10.
        b = unsafe { *data.get_unchecked(9) };
        value += u64::from(b) << 63;
        if b < 0x02 {
            return (value, 10);
        };

        // Uh oh! We've read 10 bytes and either didn't find the final byte or
        // we overflowed u64::MAX.
        (0, 11)
    }
}

impl VarIntegerTarget for u32 {
    unsafe fn decode(data: &[u8]) -> (Self, u32) {
        // Byte 1.
        let mut b: u8 = unsafe { *data.get_unchecked(0) };
        let mut value: u32 = u32::from(b);
        if b < 0x80 {
            return (value, 1);
        };
        value -= 0x80;

        // Byte 2.
        b = unsafe { *data.get_unchecked(1) };
        value += u32::from(b) << 7;
        if b < 0x80 {
            return (value, 2);
        };
        value -= 0x80 << 7;

        // Byte 3.
        b = unsafe { *data.get_unchecked(2) };
        value += u32::from(b) << 14;
        if b < 0x80 {
            return (value, 3);
        };
        value -= 0x80 << 14;

        // Byte 4.
        b = unsafe { *data.get_unchecked(3) };
        value += u32::from(b) << 21;
        if b < 0x80 {
            return (value, 4);
        };
        value -= 0x80 << 21;

        // Byte 5.
        b = unsafe { *data.get_unchecked(4) };
        value += u32::from(b) << 28;
        if b < 0x20 {
            return (value, 5);
        };

        // Uh oh! We've read 5 bytes and either didn't find the final byte or
        // we overflowed u32::MAX.
        (0, 6)
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
/// ### SAFETY:
///
/// * ValidRead: The caller must guarantee that `data` is valid for a 16-byte read.
///
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
            let part_a = std::arch::x86_64::_pext_u64(leb_part_a, 0x7f7f7f7f7f7f7f7f);
            let part_b = std::arch::x86_64::_pext_u64(leb_part_b, 0x000000000000017F);

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
    use proptest::prelude::*;
    use proptest::property_test;

    use crate::leb128::decode_u64_impl_a;

    use super::VarIntegerTarget;

    const MSB: u8 = 0b1000_0000;

    #[test]
    fn smoketest_leb128_decode_u64() {
        #[track_caller]
        fn test_case(val: u64, len: u32) {
            let mut buffer: [u8; 16] = [0u8; 16];
            leb128::write::unsigned(&mut buffer.as_mut_slice(), val).unwrap();
            // SAFETY - ValidRead: We created a buffer of 16 bytes.
            let (rnd, rnd_len) = unsafe { u64::decode(&buffer[..]) };

            assert_eq!(rnd, val, "invalid value");
            assert_eq!(len, rnd_len, "invalid length");
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
        fn test_case(val: u32, len: u32) {
            let mut buffer: [u8; 8] = [0u8; 8];
            leb128::write::unsigned(&mut buffer.as_mut_slice(), val as u64).unwrap();
            // SAFETY - ValidRead: We created a buffer of 16 bytes.
            let (rnd, rnd_len) = unsafe { u32::decode(&buffer[..]) };

            assert_eq!(rnd, val);
            assert_eq!(len, rnd_len);
        }

        test_case(0, 1);
        test_case(1, 1);
        test_case(42, 1);
        test_case(128, 2);
        test_case(u32::MAX, 5);
    }

    #[property_test]
    fn proptest_leb128_decode_u64(val: u64) {
        let mut buffer: [u8; 16] = [0u8; 16];
        leb128::write::unsigned(&mut buffer.as_mut_slice(), val).unwrap();
        let og_len = buffer
            .iter()
            .take_while(|byte| (**byte & MSB) == MSB)
            .count()
            + 1;

        // SAFETY - ValidRead: We created a buffer of 16 bytes.
        let (rnd, len) = unsafe { u64::decode(&buffer[..]) };
        prop_assert_eq!(rnd, val);
        prop_assert_eq!(len, og_len as u32);
    }

    #[property_test]
    fn proptest_leb128_decode_u32(val: u32) {
        let mut buffer: [u8; 8] = [0u8; 8];
        leb128::write::unsigned(&mut buffer.as_mut_slice(), val as u64).unwrap();
        let og_len = buffer
            .iter()
            .take_while(|byte| (**byte & MSB) == MSB)
            .count()
            + 1;

        // SAFETY - ValidRead: We created a buffer of 16 bytes.
        let (rnd, len) = unsafe { u32::decode(&buffer[..]) };
        prop_assert_eq!(rnd, val);
        prop_assert_eq!(len, og_len as u32);
    }

    #[property_test]
    fn proptest_leb128_decode_u64_a(val: u64) {
        let mut buffer: [u8; 16] = [0u8; 16];
        leb128::write::unsigned(&mut buffer.as_mut_slice(), val).unwrap();
        let og_len = buffer
            .iter()
            .take_while(|byte| (**byte & MSB) == MSB)
            .count()
            + 1;

        // Note: Despite being deadcode we continue to test decode_u64_impl_a
        // to ensure its implementation remains correct.

        // SAFETY - ValidRead: We created a buffer of 16 bytes.
        let (rnd, len) = unsafe { decode_u64_impl_a(buffer.as_ptr()) };
        prop_assert_eq!(rnd, val);
        prop_assert_eq!(len, og_len as u32);
    }
}
