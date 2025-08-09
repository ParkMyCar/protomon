use protomon::leb128::{VarIntegerTarget, decode_u64_impl_a};

const MSB: u8 = 0b1000_0000;
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
            leb128::write::unsigned(&mut buffer.as_mut_slice(), value).unwrap();
            let len = buffer
                .iter()
                .take_while(|byte| (**byte & MSB) == MSB)
                .count()
                + 1;

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
            let value = unsafe { u64::decode(data) };
            std::hint::black_box(value);
        }
    }
    let total = start.elapsed();
    println!("main {total:?}");

    let start = std::time::Instant::now();
    for _ in 0..NUM_ITERATIONS {
        for (data, _len) in &values {
            let mut read = &data[..];
            let value = unsafe { leb128::read::unsigned(&mut read).unwrap_unchecked() };
            std::hint::black_box(value);
        }
    }
    let total = start.elapsed();
    println!("leb128 crate {total:?}");
}
