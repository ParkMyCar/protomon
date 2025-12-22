use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use protomon::leb128::LebCodec;
use protomon::wire::WireType;

fn key_decoding(c: &mut Criterion) {
    let keys: Vec<_> = vec![(1u64, WireType::Varint), (43, WireType::I64)]
        .into_iter()
        .map(|(tag, wire_type)| {
            // TODO(parker): Replace this with an actual encoding function.
            let key = (tag << 3) | (wire_type.into_val() as u64);
            let mut buffer = [0u8; 16];
            key.encode_leb128(&mut buffer.as_mut_slice());
            buffer
        })
        .collect();

    let mut group = c.benchmark_group("decoding_key");
    group.bench_with_input(BenchmarkId::new("prost", keys.len()), &keys, |b, keys| {
        b.iter(|| {
            for key in keys {
                let key = prost::encoding::decode_key(&mut &key[..]);
                std::hint::black_box(key).unwrap();
            }
        })
    });
    group.bench_with_input(
        BenchmarkId::new("protomon", keys.len()),
        &keys,
        |b, keys| {
            b.iter(|| {
                for key in keys {
                    let key = protomon::wire::decode_key(&mut &key[..]);
                    std::hint::black_box(key).unwrap();
                }
            })
        },
    );
}

criterion_group!(decoding, key_decoding);

criterion_main!(decoding);
