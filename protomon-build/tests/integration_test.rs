//! Integration test for protomon-build using insta inline snapshots.

use protomon_build::Config;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_compile_simple_proto() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .compile_protos(&["tests/proto/test.proto"], &["tests/proto/"])
        .expect("Failed to compile protos");

    // Check that output files were created
    let test_rs = out_dir.path().join("test.rs");
    assert!(test_rs.exists(), "test.rs should be generated");

    let mod_rs = out_dir.path().join("mod.rs");
    assert!(mod_rs.exists(), "mod.rs should be generated");

    // Snapshot the generated code
    let content = fs::read_to_string(&test_rs).expect("Failed to read test.rs");
    insta::assert_snapshot!(content, @"
    #![allow(clippy::all)]
    #![allow(warnings)]
    #![allow(missing_docs)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(i32)]
    pub enum PhoneType {
        Mobile = 0,
        Home = 1,
        Work = 2,
    }
    impl PhoneType {
        /// Convert from i32, returning None for unknown values.
        pub fn from_i32(value: i32) -> Option<Self> {
            match value {
                0 => Some(Self::Mobile),
                1 => Some(Self::Home),
                2 => Some(Self::Work),
                _ => None,
            }
        }
    }
    impl From<PhoneType> for i32 {
        fn from(value: PhoneType) -> Self {
            value as i32
        }
    }
    impl Default for PhoneType {
        fn default() -> Self {
            Self::Mobile
        }
    }
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct Person {
        #[proto(tag = 1)]
        pub name: protomon::codec::ProtoString,
        #[proto(tag = 2)]
        pub id: i32,
        #[proto(tag = 3, optional)]
        pub email: Option<protomon::codec::ProtoString>,
        #[proto(tag = 4, repeated)]
        pub phones: protomon::codec::Repeated<PhoneNumber>,
    }
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct PhoneNumber {
        #[proto(tag = 1)]
        pub number: protomon::codec::ProtoString,
        #[proto(tag = 2)]
        pub r#type: i32,
    }
    ");

    let mod_content = fs::read_to_string(&mod_rs).expect("Failed to read mod.rs");
    insta::assert_snapshot!(mod_content, @"pub mod test;");
}

#[test]
fn test_compile_with_extensions() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .compile_protos(&["tests/proto/test_extensions.proto"], &["tests/proto/"])
        .expect("Failed to compile protos");

    let test_rs = out_dir.path().join("test_extensions.rs");
    let content = fs::read_to_string(&test_rs).expect("Failed to read test_extensions.rs");

    // Snapshot the generated code - this captures all extension behaviors:
    // - Repeated<T> vs Vec<T> for repeated fields
    // - Box<T> for boxed fields
    // - LazyMessage<T> for lazy fields
    // - [u8; N] for fixed_array bytes
    // - Vec<u8> for vec bytes
    // - Auto-boxing of recursive types
    insta::assert_snapshot!(content, @"
    #![allow(clippy::all)]
    #![allow(warnings)]
    #![allow(missing_docs)]
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct Container {
        /// Regular repeated field (uses Repeated<T>)
        #[proto(tag = 1, repeated)]
        pub regular_tags: protomon::codec::Repeated<protomon::codec::ProtoString>,
        /// Vec repeated field (uses Vec<T>)
        #[proto(tag = 2, repeated)]
        pub vec_tags: Vec<protomon::codec::ProtoString>,
        /// Boxed field (for recursive types)
        #[proto(tag = 3, optional)]
        pub child: Option<Box<Container>>,
        /// Lazy message field (uses LazyMessage<T>)
        #[proto(tag = 4, optional)]
        pub lazy_child: Option<Box<protomon::codec::LazyMessage<Container>>>,
        /// Regular message field (no LazyMessage wrapper)
        #[proto(tag = 5, optional)]
        pub eager_child: Option<Box<Container>>,
        /// Combined: lazy + boxed
        #[proto(tag = 6, optional)]
        pub lazy_boxed_child: Option<Box<protomon::codec::LazyMessage<Container>>>,
        /// Fixed-size array for bytes (e.g., SHA256 hash)
        #[proto(tag = 7)]
        pub hash: [u8; 32usize],
        /// Regular bytes field (uses ProtoBytes)
        #[proto(tag = 8)]
        pub data: protomon::codec::ProtoBytes,
        /// Fixed-size array with different size (e.g., UUID)
        #[proto(tag = 9)]
        pub uuid: [u8; 16usize],
        /// Vec<u8> for bytes field (uses Vec<u8> instead of ProtoBytes)
        #[proto(tag = 10)]
        pub vec_data: Vec<u8>,
    }
    ");
}

#[test]
fn test_recursive_type_detection() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .compile_protos(&["tests/proto/test_recursive.proto"], &["tests/proto/"])
        .expect("Failed to compile protos");

    let test_rs = out_dir.path().join("test_recursive.rs");
    let content = fs::read_to_string(&test_rs).expect("Failed to read test_recursive.rs");

    // Snapshot the generated code - this captures:
    // - Direct recursion auto-boxing (Node.left, Node.right)
    // - Indirect recursion auto-boxing (TreeA <-> TreeB cycle)
    // - Non-recursive types NOT being boxed (Leaf, Container.leaves)
    insta::assert_snapshot!(content, @"
    #![allow(clippy::all)]
    #![allow(warnings)]
    #![allow(missing_docs)]
    /// Direct recursion: a message references itself
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct Node {
        #[proto(tag = 1, optional)]
        pub value: Option<protomon::codec::ProtoString>,
        #[proto(tag = 2, optional)]
        pub left: Option<Box<Node>>,
        #[proto(tag = 3, optional)]
        pub right: Option<Box<Node>>,
    }
    /// Indirect recursion: A -> B -> A
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct TreeA {
        #[proto(tag = 1, optional)]
        pub name: Option<protomon::codec::ProtoString>,
        #[proto(tag = 2, optional)]
        pub child: Option<Box<TreeB>>,
    }
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct TreeB {
        #[proto(tag = 1, optional)]
        pub label: Option<protomon::codec::ProtoString>,
        #[proto(tag = 2, optional)]
        pub parent: Option<Box<TreeA>>,
    }
    /// No recursion: should not be boxed
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct Leaf {
        #[proto(tag = 1, optional)]
        pub data: Option<protomon::codec::ProtoString>,
        #[proto(tag = 2, optional)]
        pub count: Option<i32>,
    }
    /// References non-recursive type: should not be boxed
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct Container {
        #[proto(tag = 1, repeated)]
        pub leaves: protomon::codec::Repeated<Leaf>,
    }
    ");
}

#[test]
fn test_oneof_generation() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .compile_protos(&["tests/proto/test_oneof.proto"], &["tests/proto/"])
        .expect("Failed to compile protos");

    let test_rs = out_dir.path().join("test_oneof.rs");
    let content = fs::read_to_string(&test_rs).expect("Failed to read test_oneof.rs");

    // Snapshot the generated code - this captures:
    // - Nullable oneofs: Option<EnumType>
    // - Non-nullable oneofs: EnumType with required attribute
    // - Oneof enum generation with ProtoOneof derive
    insta::assert_snapshot!(content, @r#"
    #![allow(clippy::all)]
    #![allow(warnings)]
    #![allow(missing_docs)]
    /// Message with a nullable oneof (default)
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct NullableOneofMessage {
        #[proto(tag = 1)]
        pub name: protomon::codec::ProtoString,
        #[proto(oneof, tags = "2, 3")]
        pub widget: Option<nullable_oneof_message::Widget>,
    }
    pub mod nullable_oneof_message {
        use super::*;
        #[derive(Debug, Clone, PartialEq, protomon::ProtoOneof)]
        pub enum Widget {
            #[proto(tag = 2u32)]
            IntValue(i32),
            #[proto(tag = 3u32)]
            StringValue(protomon::codec::ProtoString),
        }
    }
    /// Message with a non-nullable (required) oneof
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct RequiredOneofMessage {
        #[proto(tag = 1)]
        pub name: protomon::codec::ProtoString,
        #[proto(oneof, tags = "2, 3", required)]
        pub widget: required_oneof_message::Widget,
    }
    pub mod required_oneof_message {
        use super::*;
        #[derive(Debug, Clone, PartialEq, protomon::ProtoOneof)]
        pub enum Widget {
            #[proto(tag = 2u32)]
            IntValue(i32),
            #[proto(tag = 3u32)]
            StringValue(protomon::codec::ProtoString),
        }
        impl Default for Widget {
            fn default() -> Self {
                Self::IntValue(Default::default())
            }
        }
    }
    /// Message with both nullable and non-nullable oneofs
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct MixedOneofMessage {
        #[proto(tag = 1)]
        pub name: protomon::codec::ProtoString,
        #[proto(oneof, tags = "2, 3")]
        pub nullable_widget: Option<mixed_oneof_message::NullableWidget>,
        #[proto(oneof, tags = "4, 5", required)]
        pub required_widget: mixed_oneof_message::RequiredWidget,
    }
    pub mod mixed_oneof_message {
        use super::*;
        #[derive(Debug, Clone, PartialEq, protomon::ProtoOneof)]
        pub enum NullableWidget {
            #[proto(tag = 2u32)]
            NullableInt(i32),
            #[proto(tag = 3u32)]
            NullableString(protomon::codec::ProtoString),
        }
        #[derive(Debug, Clone, PartialEq, protomon::ProtoOneof)]
        pub enum RequiredWidget {
            #[proto(tag = 4u32)]
            RequiredInt(i32),
            #[proto(tag = 5u32)]
            RequiredString(protomon::codec::ProtoString),
        }
        impl Default for RequiredWidget {
            fn default() -> Self {
                Self::RequiredInt(Default::default())
            }
        }
    }
    "#);
}

#[test]
fn test_map_generation() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .protoc_arg("-I../../proto") // Include protomon extensions
        .compile_protos(&["tests/proto/test_map.proto"], &["tests/proto/"])
        .expect("Failed to compile protos");

    let test_rs = out_dir.path().join("test_map.rs");
    let content = fs::read_to_string(&test_rs).expect("Failed to read test_map.rs");

    // Snapshot the generated code - this captures:
    // - Default map type (BTreeMap)
    // - Explicit btree map type
    // - HashMap with hash map type
    // - Map with message values
    // - Map with various key types
    insta::assert_snapshot!(content, @"
    #![allow(clippy::all)]
    #![allow(warnings)]
    #![allow(missing_docs)]
    /// A simple message to use as a map value.
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct Person {
        #[proto(tag = 1)]
        pub name: protomon::codec::ProtoString,
        #[proto(tag = 2)]
        pub age: i32,
    }
    /// Message with various map fields.
    #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
    pub struct MapContainer {
        /// Default map (BTreeMap)
        #[proto(tag = 1, map)]
        pub scores: alloc::collections::BTreeMap<String, i32>,
        /// Explicit BTreeMap
        #[proto(tag = 2, map)]
        pub labels: alloc::collections::BTreeMap<String, protomon::codec::ProtoString>,
        /// HashMap (requires std feature)
        #[proto(tag = 3, map)]
        pub names: std::collections::HashMap<i32, protomon::codec::ProtoString>,
        /// Map with message value
        #[proto(tag = 4, map)]
        pub people: alloc::collections::BTreeMap<String, Person>,
        /// Map with int64 key
        #[proto(tag = 5, map)]
        pub flags: alloc::collections::BTreeMap<i64, bool>,
    }
    ");
}

#[test]
fn test_comment_generation() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .compile_protos(&["tests/proto/test_comments.proto"], &["tests/proto/"])
        .expect("Failed to compile protos");

    let test_rs = out_dir.path().join("test_comments.rs");
    let content = fs::read_to_string(&test_rs).expect("Failed to read test_comments.rs");

    // Snapshot the generated code - this captures:
    // - Enum doc comments
    // - Enum value doc comments
    // - Message doc comments
    // - Field doc comments
    // - Oneof doc comments
    // - Nested message doc comments
    insta::assert_snapshot!(content);
}
