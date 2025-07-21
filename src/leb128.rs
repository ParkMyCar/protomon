/*
11100101 10001110 00100110 11110000

                           11110000 00100110 11111111 10001110 11100101
                    000000 11110000 00100110 11111111 10001110 11100101
0b100000001000000010000000 00000000 10000000 00000000 00000000 00000000
0b111111111111111111111111 01111111 11111111 01111111 01111111 01111111
0b1000000010000000100000000000000010000000000000000000000000000000
*/

/// Determines how many bytes have a most significant bit of 1, up to the first
/// byte with a most significant bit of 0.
fn leading_bytes_with_msb_1_v1(data: &[u8]) -> u32 {
    // Get at most the first 8 bytes of the slice.
    let mut bytes = [0u8; 8];
    let len = data.len().min(8);
    bytes[..len].copy_from_slice(&data[..len]);
    let value = u64::from_le_bytes(bytes);

    // Apply a mask to identify bytes where the most significant bit is zero.
    let not_msb = !value & 0x80808080_80808080u64;

    // The bytes whose most significant bit was 1 are now all zeros. This means
    // we can get our answer by counting the trailing zeros and dividing by 8.
    not_msb.trailing_zeros() >> 3
}

fn decode(data: &[u8]) -> (u64, u32) {
    // Get at most the first 8 bytes of the slice.
    //
    // N.B. We could try using `data.as_ptr().cast::<u64>.read_unaligned()` but
    // that introduces some unsafety and needs to copy the data anyways so it's
    // not entirely clear that it's the better choice.
    let mut bytes = [0u8; 8];
    let len = data.len().min(8);
    bytes[..len].copy_from_slice(&data[..len]);
    let value = u64::from_le_bytes(bytes);

    // Determine how many bytes are part of the LEB128 integer.
    //
    // 1. Apply a mask to identify bytes where the most significant bit is
    //    zero.
    // 2. The bytes whose most significant bit was 1 are now all zeros. This
    //    means we can get our answer by counting the trailing zeros and
    //    dividing by 8 (aka bit shifting right by 3).
    let not_msb = !value & 0x80808080_80808080u64;
    let leb_num_bytes = (not_msb.trailing_zeros() >> 3) + 1;

    // Mask away the bits that are not needed for the integer.
    //
    // 1. Subtract 1 from `not_msb` will invert all of the 0 bits before the
    //    first 1 bit.
    // 2. XORing against itself will turn all of the following 1 bits to 0.
    let mask = not_msb.saturating_sub(1) ^ not_msb;
    // 3. ANDing with the original value results in `leb_part` being only the
    //    bits that are part of the LEB128 encoded integer.
    let leb_part = mask & value;

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
            std::arch::x86_64::_pext_u64(leb_part, 0x7f7f7f7f7f7f7f7f)
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
            (leb_part & 0x000000000000007f)
                | ((leb_part & 0x0000000000007f00) >> 1)
                | ((leb_part & 0x00000000007f0000) >> 2)
                | ((leb_part & 0x000000007f000000) >> 3)
                | ((leb_part & 0x0000007f00000000) >> 4)
                | ((leb_part & 0x00007f0000000000) >> 5)
                | ((leb_part & 0x007f000000000000) >> 6)
                | ((leb_part & 0x7f00000000000000) >> 7)
        }
    };

    (final_value, leb_num_bytes)
}

pub trait IntegerTarget {}

impl IntegerTarget for u32 {}

impl IntegerTarget for u64 {}
