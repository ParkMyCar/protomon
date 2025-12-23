//! Benchmark comparing protomon vs prost for encode/decode.

use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

mod prost_types {
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct PhoneNumber {
        #[prost(string, tag = "1")]
        pub number: String,
        #[prost(int32, tag = "2")]
        pub phone_type: i32,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Person {
        #[prost(string, tag = "1")]
        pub name: String,
        #[prost(int32, tag = "2")]
        pub id: i32,
        #[prost(string, tag = "3")]
        pub email: String,
        #[prost(message, repeated, tag = "4")]
        pub phones: Vec<PhoneNumber>,
    }
}

mod protomon_types {
    use protomon::codec::{LazyMessage, ProtoString, Repeated};
    use protomon::ProtoMessage;

    #[derive(Clone, PartialEq, Default, ProtoMessage)]
    pub struct PhoneNumber {
        #[proto(tag = 1)]
        pub number: ProtoString,
        #[proto(tag = 2)]
        pub phone_type: i32,
    }

    #[derive(Clone, Default, ProtoMessage)]
    pub struct Person {
        #[proto(tag = 1)]
        pub name: ProtoString,
        #[proto(tag = 2)]
        pub id: i32,
        #[proto(tag = 3)]
        pub email: ProtoString,
        #[proto(tag = 4, repeated)]
        pub phones: Repeated<PhoneNumber>,
    }

    /// Person variant using Vec<LazyMessage<PhoneNumber>> instead of Repeated
    #[derive(Clone, Default, ProtoMessage)]
    pub struct PersonVecLazy {
        #[proto(tag = 1)]
        pub name: ProtoString,
        #[proto(tag = 2)]
        pub id: i32,
        #[proto(tag = 3)]
        pub email: ProtoString,
        #[proto(tag = 4, repeated)]
        pub phones: Vec<LazyMessage<PhoneNumber>>,
    }

    /// Person variant using Vec<PhoneNumber> - fully eager like prost
    #[derive(Clone, Default, ProtoMessage)]
    pub struct PersonVecEager {
        #[proto(tag = 1)]
        pub name: ProtoString,
        #[proto(tag = 2)]
        pub id: i32,
        #[proto(tag = 3)]
        pub email: ProtoString,
        #[proto(tag = 4, repeated)]
        pub phones: Vec<PhoneNumber>,
    }

    // ========== String-based types (1:1 with prost) ==========

    #[derive(Clone, PartialEq, Default, ProtoMessage)]
    pub struct PhoneNumberStr {
        #[proto(tag = 1)]
        pub number: String,
        #[proto(tag = 2)]
        pub phone_type: i32,
    }

    /// Person using String instead of ProtoString - 1:1 comparison with prost
    #[derive(Clone, Default, ProtoMessage)]
    pub struct PersonStr {
        #[proto(tag = 1)]
        pub name: String,
        #[proto(tag = 2)]
        pub id: i32,
        #[proto(tag = 3)]
        pub email: String,
        #[proto(tag = 4, repeated)]
        pub phones: Vec<PhoneNumberStr>,
    }
}

fn create_prost_person(num_phones: usize) -> prost_types::Person {
    prost_types::Person {
        name: "Alice Johnson".to_string(),
        id: 12345,
        email: "alice.johnson@example.com".to_string(),
        phones: (0..num_phones)
            .map(|i| prost_types::PhoneNumber {
                number: format!("555-{:04}", i),
                phone_type: (i % 3) as i32,
            })
            .collect(),
    }
}

fn create_protomon_person(num_phones: usize) -> protomon_types::Person {
    use protomon::codec::{ProtoString, Repeated};

    let phones = Repeated::owned(
        (0..num_phones)
            .map(|i| protomon_types::PhoneNumber {
                number: ProtoString::from(format!("555-{:04}", i)),
                phone_type: (i % 3) as i32,
            })
            .collect(),
    );

    protomon_types::Person {
        name: ProtoString::from("Alice Johnson"),
        id: 12345,
        email: ProtoString::from("alice.johnson@example.com"),
        phones,
    }
}

fn encode_benchmark(c: &mut Criterion) {
    use prost::Message as _;
    use protomon::codec::ProtoMessage as _;

    let mut group = c.benchmark_group("encode");

    for num_phones in [0, 1, 10, 100] {
        let prost_person = create_prost_person(num_phones);
        let protomon_person = create_protomon_person(num_phones);

        // Calculate encoded size for throughput
        let encoded_size = prost_person.encoded_len();
        group.throughput(Throughput::Bytes(encoded_size as u64));

        group.bench_with_input(
            BenchmarkId::new("prost", num_phones),
            &prost_person,
            |b, person| {
                b.iter(|| {
                    let mut buf = Vec::with_capacity(person.encoded_len());
                    person.encode(&mut buf).unwrap();
                    std::hint::black_box(buf)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("protomon", num_phones),
            &protomon_person,
            |b, person| {
                b.iter(|| {
                    let mut buf = Vec::with_capacity(person.encoded_message_len());
                    person.encode_message(&mut buf);
                    std::hint::black_box(buf)
                })
            },
        );
    }

    group.finish();
}

fn decode_benchmark(c: &mut Criterion) {
    use prost::Message as _;
    use protomon::codec::ProtoMessage as _;

    let mut group = c.benchmark_group("decode");

    for num_phones in [0, 1, 10, 100] {
        // Encode with prost to get wire-compatible bytes
        let prost_person = create_prost_person(num_phones);
        let mut encoded = Vec::new();
        prost_person.encode(&mut encoded).unwrap();
        let encoded_bytes = Bytes::from(encoded.clone());

        group.throughput(Throughput::Bytes(encoded.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("prost", num_phones),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let person = prost_types::Person::decode(&data[..]).unwrap();
                    std::hint::black_box(person)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("protomon", num_phones),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let person = protomon_types::Person::decode_message(data.clone()).unwrap();
                    std::hint::black_box(person)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("protomon_string", num_phones),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let person =
                        protomon_types::PersonStr::decode_message(data.clone()).unwrap();
                    std::hint::black_box(person)
                })
            },
        );
    }

    group.finish();
}

fn decode_and_access_benchmark(c: &mut Criterion) {
    use prost::Message as _;
    use protomon::codec::ProtoMessage as _;

    let mut group = c.benchmark_group("decode_and_access");

    for num_phones in [0, 1, 10, 100] {
        // Encode with prost to get wire-compatible bytes
        let prost_person = create_prost_person(num_phones);
        let mut encoded = Vec::new();
        prost_person.encode(&mut encoded).unwrap();
        let encoded_bytes = Bytes::from(encoded.clone());

        group.throughput(Throughput::Bytes(encoded.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("prost", num_phones),
            &encoded,
            |b, data| {
                b.iter(|| {
                    let person = prost_types::Person::decode(&data[..]).unwrap();
                    // Access all fields to ensure full decode
                    let mut sum = person.name.len() + person.email.len() + person.id as usize;
                    for phone in &person.phones {
                        sum += phone.number.len() + phone.phone_type as usize;
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("protomon_repeated", num_phones),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let person = protomon_types::Person::decode_message(data.clone()).unwrap();
                    // Access all fields - this triggers lazy decoding via Repeated
                    let mut sum = person.name.len() + person.email.len() + person.id as usize;
                    for phone in person.phones.iter().flatten() {
                        sum += phone.number.len() + phone.phone_type as usize;
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("protomon_vec_lazy", num_phones),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let person =
                        protomon_types::PersonVecLazy::decode_message(data.clone()).unwrap();
                    // Access all fields - Vec<LazyMessage> decodes on access
                    let mut sum = person.name.len() + person.email.len() + person.id as usize;
                    for lazy_phone in &person.phones {
                        let phone = lazy_phone.decode().unwrap();
                        sum += phone.number.len() + phone.phone_type as usize;
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("protomon_vec_eager", num_phones),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let person =
                        protomon_types::PersonVecEager::decode_message(data.clone()).unwrap();
                    // Access all fields - Vec<PhoneNumber> is fully decoded
                    let mut sum = person.name.len() + person.email.len() + person.id as usize;
                    for phone in &person.phones {
                        sum += phone.number.len() + phone.phone_type as usize;
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("protomon_str", num_phones),
            &encoded_bytes,
            |b, data| {
                b.iter(|| {
                    let person = protomon_types::PersonStr::decode_message(data.clone()).unwrap();
                    // Access all fields - String types, 1:1 with prost
                    let mut sum = person.name.len() + person.email.len() + person.id as usize;
                    for phone in &person.phones {
                        sum += phone.number.len() + phone.phone_type as usize;
                    }
                    std::hint::black_box(sum)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    encode_benchmark,
    decode_benchmark,
    decode_and_access_benchmark
);
criterion_main!(benches);
