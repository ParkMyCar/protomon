//! Benchmarks for packed repeated field decoding.
//!
//! Tests decoding performance for various packed field types and sizes.
//! Compares protomon's PackedDecode against prost.

use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use protomon::codec::{Fixed32, Fixed64, PackedDecode, PackedIter, ProtoEncode, Sfixed32, Sfixed64};

/// Generate packed encoded data for a given type (raw, no length prefix).
fn encode_packed<T: ProtoEncode + Clone>(values: &[T]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(values.len() * 8);
    for v in values {
        v.encode(&mut buf);
    }
    buf
}

/// Generate packed encoded data with a length prefix (for prost).
fn encode_packed_with_length<T: ProtoEncode + Clone>(values: &[T]) -> Vec<u8> {
    let raw = encode_packed(values);
    let mut buf = Vec::with_capacity(raw.len() + 5);
    prost::encoding::encode_varint(raw.len() as u64, &mut buf);
    buf.extend_from_slice(&raw);
    buf
}

/// Benchmark decoding packed fixed32 fields.
fn bench_packed_fixed32(c: &mut Criterion) {
    let mut group = c.benchmark_group("packed_fixed32");

    for count in [10, 100, 1000, 10000] {
        let values: Vec<Fixed32> = (0..count).map(|i| Fixed32(i as u32)).collect();
        let encoded = encode_packed(&values);
        let encoded_bytes = Bytes::from(encoded.clone());
        let encoded_with_len = encode_packed_with_length(&values);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("PackedIter", count),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<Fixed32> = PackedIter::new(data.clone());
                    let collected: Vec<Fixed32> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized: PackedDecode trait implementation
        group.bench_with_input(
            BenchmarkId::new("PackedDecode", count),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = Fixed32::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison
        group.bench_with_input(
            BenchmarkId::new("prost", count),
            &encoded_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::fixed32::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );

        // Baseline: direct slice iteration (what SIMD should approach)
        group.bench_with_input(
            BenchmarkId::new("direct_slice", count),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let slice = &data[..];
                    let count = slice.len() / 4;
                    let mut result = Vec::with_capacity(count);
                    for i in 0..count {
                        let bytes = &slice[i * 4..(i + 1) * 4];
                        let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                        result.push(Fixed32(value));
                    }
                    std::hint::black_box(result)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark decoding packed fixed64 fields.
fn bench_packed_fixed64(c: &mut Criterion) {
    let mut group = c.benchmark_group("packed_fixed64");

    for count in [10, 100, 1000, 10000] {
        let values: Vec<Fixed64> = (0..count).map(|i| Fixed64(i as u64)).collect();
        let encoded = encode_packed(&values);
        let encoded_bytes = Bytes::from(encoded.clone());
        let encoded_with_len = encode_packed_with_length(&values);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("PackedIter", count),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<Fixed64> = PackedIter::new(data.clone());
                    let collected: Vec<Fixed64> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized: PackedDecode trait implementation
        group.bench_with_input(
            BenchmarkId::new("PackedDecode", count),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = Fixed64::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison
        group.bench_with_input(
            BenchmarkId::new("prost", count),
            &encoded_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::fixed64::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );

        // Baseline: direct slice iteration
        group.bench_with_input(
            BenchmarkId::new("direct_slice", count),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let slice = &data[..];
                    let count = slice.len() / 8;
                    let mut result = Vec::with_capacity(count);
                    for i in 0..count {
                        let bytes = &slice[i * 8..(i + 1) * 8];
                        let value = u64::from_le_bytes([
                            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                            bytes[7],
                        ]);
                        result.push(Fixed64(value));
                    }
                    std::hint::black_box(result)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark decoding packed f32 fields.
fn bench_packed_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("packed_f32");

    for count in [10, 100, 1000, 10000] {
        let values: Vec<f32> = (0..count).map(|i| i as f32 * 1.5).collect();
        let encoded = encode_packed(&values);
        let encoded_bytes = Bytes::from(encoded.clone());
        let encoded_with_len = encode_packed_with_length(&values);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("PackedIter", count),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<f32> = PackedIter::new(data.clone());
                    let collected: Vec<f32> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized: PackedDecode trait implementation
        group.bench_with_input(
            BenchmarkId::new("PackedDecode", count),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = f32::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison
        group.bench_with_input(
            BenchmarkId::new("prost", count),
            &encoded_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::float::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark decoding packed f64 fields.
fn bench_packed_f64(c: &mut Criterion) {
    let mut group = c.benchmark_group("packed_f64");

    for count in [10, 100, 1000, 10000] {
        let values: Vec<f64> = (0..count).map(|i| i as f64 * 1.5).collect();
        let encoded = encode_packed(&values);
        let encoded_bytes = Bytes::from(encoded.clone());
        let encoded_with_len = encode_packed_with_length(&values);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("PackedIter", count),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<f64> = PackedIter::new(data.clone());
                    let collected: Vec<f64> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized: PackedDecode trait implementation
        group.bench_with_input(
            BenchmarkId::new("PackedDecode", count),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = f64::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison
        group.bench_with_input(
            BenchmarkId::new("prost", count),
            &encoded_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::double::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark decoding packed u32 (varint) fields.
fn bench_packed_u32(c: &mut Criterion) {
    let mut group = c.benchmark_group("packed_u32_varint");

    for count in [10, 100, 1000, 10000] {
        // Test with small values (1-byte varints)
        let small_values: Vec<u32> = (0..count).map(|i| (i % 127) as u32).collect();
        let small_encoded = encode_packed(&small_values);
        let small_bytes = Bytes::from(small_encoded.clone());
        let small_with_len = encode_packed_with_length(&small_values);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("small_PackedIter", count),
            &small_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<u32> = PackedIter::new(data.clone());
                    let collected: Vec<u32> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized PackedDecode for small values
        group.bench_with_input(
            BenchmarkId::new("small_PackedDecode", count),
            &small_encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = u32::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison for small values
        group.bench_with_input(
            BenchmarkId::new("small_prost", count),
            &small_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::uint32::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );

        // Test with large values (5-byte varints)
        let large_values: Vec<u32> = (0..count).map(|i| u32::MAX - i as u32).collect();
        let large_encoded = encode_packed(&large_values);
        let large_bytes = Bytes::from(large_encoded.clone());
        let large_with_len = encode_packed_with_length(&large_values);

        group.bench_with_input(
            BenchmarkId::new("large_PackedIter", count),
            &large_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<u32> = PackedIter::new(data.clone());
                    let collected: Vec<u32> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized PackedDecode for large values
        group.bench_with_input(
            BenchmarkId::new("large_PackedDecode", count),
            &large_encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = u32::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison for large values
        group.bench_with_input(
            BenchmarkId::new("large_prost", count),
            &large_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::uint32::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );

    }

    group.finish();
}

/// Benchmark decoding packed u64 (varint) fields.
fn bench_packed_u64(c: &mut Criterion) {
    let mut group = c.benchmark_group("packed_u64_varint");

    for count in [10, 100, 1000, 10000] {
        // Small values
        let small_values: Vec<u64> = (0..count).map(|i| (i % 127) as u64).collect();
        let small_encoded = encode_packed(&small_values);
        let small_bytes = Bytes::from(small_encoded.clone());
        let small_with_len = encode_packed_with_length(&small_values);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("small_PackedIter", count),
            &small_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<u64> = PackedIter::new(data.clone());
                    let collected: Vec<u64> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized PackedDecode for small values
        group.bench_with_input(
            BenchmarkId::new("small_PackedDecode", count),
            &small_encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = u64::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison for small values
        group.bench_with_input(
            BenchmarkId::new("small_prost", count),
            &small_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::uint64::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );

        // Large values (10-byte varints)
        let large_values: Vec<u64> = (0..count).map(|i| u64::MAX - i as u64).collect();
        let large_encoded = encode_packed(&large_values);
        let large_bytes = Bytes::from(large_encoded.clone());
        let large_with_len = encode_packed_with_length(&large_values);

        group.bench_with_input(
            BenchmarkId::new("large_PackedIter", count),
            &large_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<u64> = PackedIter::new(data.clone());
                    let collected: Vec<u64> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized PackedDecode for large values
        group.bench_with_input(
            BenchmarkId::new("large_PackedDecode", count),
            &large_encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = u64::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison for large values
        group.bench_with_input(
            BenchmarkId::new("large_prost", count),
            &large_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::uint64::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark decoding packed sfixed32 fields.
fn bench_packed_sfixed32(c: &mut Criterion) {
    let mut group = c.benchmark_group("packed_sfixed32");

    for count in [10, 100, 1000, 10000] {
        let values: Vec<Sfixed32> = (0..count)
            .map(|i| Sfixed32(if i % 2 == 0 { i as i32 } else { -(i as i32) }))
            .collect();
        let encoded = encode_packed(&values);
        let encoded_bytes = Bytes::from(encoded.clone());
        let encoded_with_len = encode_packed_with_length(&values);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("PackedIter", count),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<Sfixed32> = PackedIter::new(data.clone());
                    let collected: Vec<Sfixed32> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized: PackedDecode trait implementation
        group.bench_with_input(
            BenchmarkId::new("PackedDecode", count),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = Sfixed32::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison
        group.bench_with_input(
            BenchmarkId::new("prost", count),
            &encoded_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::sfixed32::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark decoding packed sfixed64 fields.
fn bench_packed_sfixed64(c: &mut Criterion) {
    let mut group = c.benchmark_group("packed_sfixed64");

    for count in [10, 100, 1000, 10000] {
        let values: Vec<Sfixed64> = (0..count)
            .map(|i| Sfixed64(if i % 2 == 0 { i as i64 } else { -(i as i64) }))
            .collect();
        let encoded = encode_packed(&values);
        let encoded_bytes = Bytes::from(encoded.clone());
        let encoded_with_len = encode_packed_with_length(&values);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("PackedIter", count),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let iter: PackedIter<Sfixed64> = PackedIter::new(data.clone());
                    let collected: Vec<Sfixed64> = iter.map(|r| r.unwrap()).collect();
                    std::hint::black_box(collected)
                })
            },
        );

        // Optimized: PackedDecode trait implementation
        group.bench_with_input(
            BenchmarkId::new("PackedDecode", count),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let decoded = Sfixed64::decode_packed(data).unwrap();
                    std::hint::black_box(decoded)
                })
            },
        );

        // Prost comparison
        group.bench_with_input(
            BenchmarkId::new("prost", count),
            &encoded_with_len,
            |b, data| {
                b.iter(|| {
                    let mut values = Vec::new();
                    let mut buf = &data[..];
                    prost::encoding::sfixed64::merge_repeated(
                        prost::encoding::WireType::LengthDelimited,
                        &mut values,
                        &mut buf,
                        prost::encoding::DecodeContext::default(),
                    )
                    .unwrap();
                    std::hint::black_box(values)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    packed_benches,
    bench_packed_fixed32,
    bench_packed_fixed64,
    bench_packed_f32,
    bench_packed_f64,
    bench_packed_u32,
    bench_packed_u64,
    bench_packed_sfixed32,
    bench_packed_sfixed64,
);

criterion_main!(packed_benches);
