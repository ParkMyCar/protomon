use protomon::leb128::{VarIntegerTarget, decode_u64_impl_a};

const NUM_ITERATIONS: usize = 1_000_000_000;

// N.B. These are purposefully out of order to confuse branch predictors.
static LEB128_VALUES: [u64; 5] = [
    0x0000_0000_8000_0000,
    0x0000_0000_0000_8000,
    1,
    0x0000_0000_0000_0080,
    0x0000_0000_0080_0000,
];

pub fn main() {
    let values: Vec<_> = LEB128_VALUES
        .into_iter()
        .map(|value| {
            let mut buffer: [u8; 16] = [0u8; 16];
            let len = value.encode_leb128(&mut buffer.as_mut_slice());
            (buffer, len)
        })
        .collect();

    // Warmup.
    let start = std::time::Instant::now();
    let mut num_values = 0;
    for _ in 0..NUM_ITERATIONS {
        num_values += LEB128_VALUES.len();
    }
    let total = start.elapsed();
    println!("warmup {total:?}, num values {num_values}");

    // Implementations.

    let start = std::time::Instant::now();
    for _ in 0..NUM_ITERATIONS {
        for (data, _len) in &values {
            let value = unsafe { decode_u64_impl_a(data.as_ptr()) };
            std::hint::black_box(value);
        }
    }
    let total = start.elapsed();
    println!("a {total:?}");

    let start = std::time::Instant::now();
    for _ in 0..NUM_ITERATIONS {
        for (data, _len) in &values {
            let value = unsafe { u64::decode_leb128(data) };
            std::hint::black_box(value);
        }
    }
    let total = start.elapsed();
    println!("main {total:?}");
}
