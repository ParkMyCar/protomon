use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use protomon::leb128::{VarIntegerTarget, decode_u64_impl_a};

const MSB: u8 = 0b1000_0000;

fn leb128_decoding_single(c: &mut Criterion) {
    let values: Vec<_> = vec![
        1,
        0x0000_0000_0000_0080,
        0x0000_0000_0000_8000,
        0x0000_0000_0080_0000,
        0x0000_0000_8000_0000,
        // 0x0000_0080_0000_0000,
        // 0x0000_8000_0000_0000,
        // 0x0080_0000_0000_0000,
        // 0x8000_0000_0000_0000,
    ]
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

    let mut group = c.benchmark_group("decoding_single");
    for (data, len) in &values {
        group.bench_with_input(BenchmarkId::new("protomon", len), &data, |b, data| {
            b.iter(|| {
                let value = unsafe { u64::decode(&data[..]) };
                std::hint::black_box(value)
            })
        });
        group.bench_with_input(
            BenchmarkId::new("protomon impl_a", len),
            &data,
            |b, data| {
                b.iter(|| {
                    let value = unsafe { decode_u64_impl_a(data.as_ptr()) };
                    std::hint::black_box(value)
                })
            },
        );
        group.bench_with_input(BenchmarkId::new("leb128", len), &data, |b, data| {
            b.iter(|| {
                let value = leb128::read::unsigned(&mut data.as_slice()).unwrap();
                std::hint::black_box(value)
            })
        });
    }
}

fn leb128_decoding_many(c: &mut Criterion) {
    let values: Vec<_> = vec![
        1,
        0x0000_0000_0000_0080,
        0x0000_0000_0000_8000,
        0x0000_0000_0080_0000,
        0x0000_0000_8000_0000,
        // 0x0000_0080_0000_0000,
        // 0x0000_8000_0000_0000,
        // 0x0080_0000_0000_0000,
        // 0x8000_0000_0000_0000,
    ]
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

    let mut group = c.benchmark_group("decoding_many");
    group.bench_with_input(
        BenchmarkId::new("protomon", values.len()),
        &values,
        |b, data| {
            b.iter(|| {
                for (value, _len) in data {
                    let value = unsafe { u64::decode(&value[..]) };
                    std::hint::black_box(value);
                }
            });
        },
    );
    group.bench_with_input(
        BenchmarkId::new("protomon impl_a", values.len()),
        &values,
        |b, data| {
            b.iter(|| {
                for (value, _len) in data {
                    let value = unsafe { decode_u64_impl_a(value.as_ptr()) };
                    std::hint::black_box(value);
                }
            });
        },
    );
    group.bench_with_input(
        BenchmarkId::new("leb128", values.len()),
        &values,
        |b, data| {
            b.iter(|| {
                for (value, _len) in data {
                    let value = leb128::read::unsigned(&mut value.as_slice()).unwrap();
                    std::hint::black_box(value);
                }
            });
        },
    );
}
criterion_group!(decoding, leb128_decoding_single, leb128_decoding_many);

criterion_main!(decoding);
