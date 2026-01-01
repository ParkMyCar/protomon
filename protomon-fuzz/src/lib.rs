//! Fuzzing framework for protomon.
//!
//! This crate generates random protobuf schemas and values for fuzz testing
//! the protomon encoding/decoding implementation against other protobuf
//! implementations (C++, Go).
//!
//! # Example
//!
//! ```
//! use protomon_fuzz::{TestCase, Schema};
//! use arbitrary::Unstructured;
//!
//! // Generate a random test case from arbitrary bytes
//! let data = [0u8; 64];
//! let mut u = Unstructured::new(&data);
//! if let Ok(test_case) = TestCase::arbitrary(&mut u) {
//!     // Output the .proto schema
//!     println!("{}", test_case.to_proto());
//!
//!     // Output JSON for each message
//!     for (name, json) in test_case.to_json_files() {
//!         println!("--- {} ---\n{}", name, json);
//!     }
//! }
//! ```

mod value;

pub use value::{FieldValue, MessageValue, ScalarValue, TestCase};

use arbitrary::{Arbitrary, Unstructured};
use std::collections::HashSet;
use std::fmt;

/// Maximum number of fields per message.
const MAX_FIELDS_PER_MESSAGE: usize = 16;

/// Maximum number of nested messages per message.
const MAX_NESTED_MESSAGES: usize = 4;

/// Maximum nesting depth for messages.
const MAX_NESTING_DEPTH: usize = 3;

/// Minimum valid protobuf field number.
pub const MIN_FIELD_NUMBER: u32 = 1;

/// Maximum valid protobuf field number.
/// Reserved range 19000-19999 is excluded, and max is 2^29 - 1.
pub const MAX_FIELD_NUMBER: u32 = 536_870_911;

/// Scalar types supported in protobuf.
///
/// These map directly to the protobuf scalar value types.
/// See: https://protobuf.dev/programming-guides/proto3/#scalar
#[derive(Debug, Clone, Copy, PartialEq, Eq, Arbitrary)]
pub enum ScalarType {
    // Varint types (WireType::Varint)
    Int32,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Bool,

    // Fixed 32-bit types (WireType::I32)
    Fixed32,
    Sfixed32,
    Float,

    // Fixed 64-bit types (WireType::I64)
    Fixed64,
    Sfixed64,
    Double,

    // Length-delimited types (WireType::Len)
    String,
    Bytes,
}

impl ScalarType {
    /// Returns the protobuf type name for this scalar.
    pub fn proto_name(&self) -> &'static str {
        match self {
            ScalarType::Int32 => "int32",
            ScalarType::Int64 => "int64",
            ScalarType::Uint32 => "uint32",
            ScalarType::Uint64 => "uint64",
            ScalarType::Sint32 => "sint32",
            ScalarType::Sint64 => "sint64",
            ScalarType::Bool => "bool",
            ScalarType::Fixed32 => "fixed32",
            ScalarType::Sfixed32 => "sfixed32",
            ScalarType::Float => "float",
            ScalarType::Fixed64 => "fixed64",
            ScalarType::Sfixed64 => "sfixed64",
            ScalarType::Double => "double",
            ScalarType::String => "string",
            ScalarType::Bytes => "bytes",
        }
    }

    /// Returns the Rust type that corresponds to this protobuf scalar.
    pub fn rust_type(&self) -> &'static str {
        match self {
            ScalarType::Int32 => "i32",
            ScalarType::Int64 => "i64",
            ScalarType::Uint32 => "u32",
            ScalarType::Uint64 => "u64",
            ScalarType::Sint32 => "Sint32",
            ScalarType::Sint64 => "Sint64",
            ScalarType::Bool => "bool",
            ScalarType::Fixed32 => "Fixed32",
            ScalarType::Sfixed32 => "Sfixed32",
            ScalarType::Float => "f32",
            ScalarType::Fixed64 => "Fixed64",
            ScalarType::Sfixed64 => "Sfixed64",
            ScalarType::Double => "f64",
            ScalarType::String => "ProtoString",
            ScalarType::Bytes => "ProtoBytes",
        }
    }
}

impl fmt::Display for ScalarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.proto_name())
    }
}

/// Field cardinality in protobuf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Arbitrary)]
pub enum FieldCardinality {
    /// A singular field (proto3 implicit presence).
    Singular,
    /// An optional field (explicit presence).
    Optional,
    /// A repeated field.
    Repeated,
}

impl FieldCardinality {
    /// Returns whether this field can appear multiple times.
    pub fn is_repeated(&self) -> bool {
        matches!(self, FieldCardinality::Repeated)
    }
}

/// The type of a field - either a scalar or a reference to a nested message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    /// A scalar type like int32, string, etc.
    Scalar(ScalarType),
    /// A reference to a nested message by index.
    /// The index refers to the position in the parent message's nested_messages vec.
    Message(usize),
}

/// A field descriptor within a protobuf message.
#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    /// The field name (e.g., "user_id").
    pub name: String,
    /// The field number (1 to 2^29-1, excluding reserved range).
    pub number: u32,
    /// The type of this field.
    pub field_type: FieldType,
    /// The cardinality (singular, optional, repeated).
    pub cardinality: FieldCardinality,
}

impl FieldDescriptor {
    /// Generate a field name from a seed value.
    fn generate_name(seed: u8) -> String {
        // Simple field name generation - produces names like "field_a", "field_b", etc.
        let suffix = (b'a' + (seed % 26)) as char;
        format!("field_{}", suffix)
    }

    /// Generate a valid field number, avoiding the reserved range 19000-19999.
    fn generate_field_number(seed: u32, used_numbers: &HashSet<u32>) -> u32 {
        // Start from seed and find the next available number
        let mut candidate = (seed % 1000) + 1; // Keep numbers small for readability
        while used_numbers.contains(&candidate) || (19000..=19999).contains(&candidate) {
            candidate += 1;
            if candidate > 20000 {
                candidate = 1;
            }
        }
        candidate
    }
}

/// A message descriptor representing a protobuf message type.
#[derive(Debug, Clone)]
pub struct MessageDescriptor {
    /// The message name (e.g., "UserRequest").
    pub name: String,
    /// Fields within this message.
    pub fields: Vec<FieldDescriptor>,
    /// Nested message types defined within this message.
    pub nested_messages: Vec<MessageDescriptor>,
}

impl MessageDescriptor {
    /// Generate a message name from a seed and depth.
    fn generate_name(seed: u8, depth: usize) -> String {
        let prefix = match depth {
            0 => "Root",
            1 => "Nested",
            2 => "Inner",
            _ => "Deep",
        };
        let suffix = (b'A' + (seed % 26)) as char;
        format!("{}Message{}", prefix, suffix)
    }

    /// Render this message as protobuf syntax.
    pub fn to_proto(&self, indent: usize) -> String {
        self.to_proto_with_syntax(indent, ProtobufSyntax::Proto3)
    }

    /// Render this message as protobuf syntax with explicit syntax version.
    pub fn to_proto_with_syntax(&self, indent: usize, syntax: ProtobufSyntax) -> String {
        let mut output = String::new();
        let indent_str = "  ".repeat(indent);

        output.push_str(&format!("{}message {} {{\n", indent_str, self.name));

        // Render nested messages first
        for nested in &self.nested_messages {
            output.push_str(&nested.to_proto_with_syntax(indent + 1, syntax));
            output.push('\n');
        }

        // Render fields
        for field in &self.fields {
            // In proto2, all fields need explicit modifiers.
            // In proto3, singular fields have no prefix, optional/repeated are explicit.
            let cardinality_str = match (syntax, field.cardinality) {
                (ProtobufSyntax::Proto3, FieldCardinality::Singular) => "",
                (ProtobufSyntax::Proto2, FieldCardinality::Singular) => "optional ",
                (_, FieldCardinality::Optional) => "optional ",
                (_, FieldCardinality::Repeated) => "repeated ",
            };

            let type_str = match &field.field_type {
                FieldType::Scalar(scalar) => scalar.proto_name().to_string(),
                FieldType::Message(idx) => {
                    if *idx < self.nested_messages.len() {
                        self.nested_messages[*idx].name.clone()
                    } else {
                        "UnknownMessage".to_string()
                    }
                }
            };

            output.push_str(&format!(
                "{}  {}{} {} = {};\n",
                indent_str, cardinality_str, type_str, field.name, field.number
            ));
        }

        output.push_str(&format!("{}}}", indent_str));
        output
    }
}

/// A complete protobuf schema with one or more message types.
#[derive(Debug, Clone)]
pub struct Schema {
    /// The package name (e.g., "fuzz.test").
    pub package: String,
    /// Top-level message types in this schema.
    pub messages: Vec<MessageDescriptor>,
    /// Use proto3 syntax.
    pub syntax: ProtobufSyntax,
}

/// Protobuf syntax version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Arbitrary)]
pub enum ProtobufSyntax {
    Proto2,
    Proto3,
}

impl fmt::Display for ProtobufSyntax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtobufSyntax::Proto2 => write!(f, "proto2"),
            ProtobufSyntax::Proto3 => write!(f, "proto3"),
        }
    }
}

impl Schema {
    /// Render this schema as a complete .proto file.
    pub fn to_proto(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("syntax = \"{}\";\n\n", self.syntax));
        output.push_str(&format!("package {};\n\n", self.package));

        for message in &self.messages {
            output.push_str(&message.to_proto_with_syntax(0, self.syntax));
            output.push_str("\n\n");
        }

        output
    }

    /// Returns the total number of message types (including nested).
    pub fn total_message_count(&self) -> usize {
        fn count_messages(msg: &MessageDescriptor) -> usize {
            1 + msg
                .nested_messages
                .iter()
                .map(count_messages)
                .sum::<usize>()
        }
        self.messages.iter().map(count_messages).sum()
    }

    /// Returns the total number of fields across all messages.
    pub fn total_field_count(&self) -> usize {
        fn count_fields(msg: &MessageDescriptor) -> usize {
            msg.fields.len() + msg.nested_messages.iter().map(count_fields).sum::<usize>()
        }
        self.messages.iter().map(count_fields).sum()
    }
}

impl<'a> Arbitrary<'a> for Schema {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let syntax: ProtobufSyntax = u.arbitrary()?;

        // Generate 1-3 top-level messages
        let num_messages = u.int_in_range(1..=3)?;
        let mut messages = Vec::with_capacity(num_messages);

        for i in 0..num_messages {
            let msg = generate_message(u, i as u8, 0)?;
            messages.push(msg);
        }

        Ok(Schema {
            package: "fuzz.test".to_string(),
            messages,
            syntax,
        })
    }
}

/// Generate a message descriptor with random fields and optionally nested messages.
fn generate_message(
    u: &mut Unstructured<'_>,
    seed: u8,
    depth: usize,
) -> arbitrary::Result<MessageDescriptor> {
    let name = MessageDescriptor::generate_name(seed, depth);

    // Generate nested messages first (if not too deep)
    let mut nested_messages = Vec::new();
    if depth < MAX_NESTING_DEPTH {
        let num_nested = u.int_in_range(0..=MAX_NESTED_MESSAGES.min(2))?;
        for i in 0..num_nested {
            let nested = generate_message(u, seed.wrapping_add(i as u8 + 1), depth + 1)?;
            nested_messages.push(nested);
        }
    }

    // Generate fields
    let num_fields = u.int_in_range(1..=MAX_FIELDS_PER_MESSAGE)?;
    let mut fields = Vec::with_capacity(num_fields);
    let mut used_numbers = HashSet::new();
    let mut used_names = HashSet::new();

    for i in 0..num_fields {
        let field = generate_field(
            u,
            i as u8,
            &nested_messages,
            &mut used_numbers,
            &mut used_names,
        )?;
        fields.push(field);
    }

    Ok(MessageDescriptor {
        name,
        fields,
        nested_messages,
    })
}

/// Generate a field descriptor with random type and cardinality.
fn generate_field(
    u: &mut Unstructured<'_>,
    seed: u8,
    nested_messages: &[MessageDescriptor],
    used_numbers: &mut HashSet<u32>,
    used_names: &mut HashSet<String>,
) -> arbitrary::Result<FieldDescriptor> {
    // Generate unique name
    let mut name = FieldDescriptor::generate_name(seed);
    let mut name_seed = seed;
    while used_names.contains(&name) {
        name_seed = name_seed.wrapping_add(1);
        name = FieldDescriptor::generate_name(name_seed);
    }
    used_names.insert(name.clone());

    // Generate unique field number
    let number_seed: u32 = u.arbitrary()?;
    let number = FieldDescriptor::generate_field_number(number_seed, used_numbers);
    used_numbers.insert(number);

    // Decide if this should be a scalar or message type
    let use_message_type = !nested_messages.is_empty() && u.ratio(1, 4)?;

    let field_type = if use_message_type {
        let idx = u.int_in_range(0..=nested_messages.len() - 1)?;
        FieldType::Message(idx)
    } else {
        FieldType::Scalar(u.arbitrary()?)
    };

    let cardinality: FieldCardinality = u.arbitrary()?;

    Ok(FieldDescriptor {
        name,
        number,
        field_type,
        cardinality,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_scalar_type_names() {
        let names: Vec<_> = [
            ScalarType::Int32,
            ScalarType::Int64,
            ScalarType::Uint32,
            ScalarType::Uint64,
            ScalarType::Sint32,
            ScalarType::Sint64,
            ScalarType::Bool,
            ScalarType::Fixed32,
            ScalarType::Sfixed32,
            ScalarType::Float,
            ScalarType::Fixed64,
            ScalarType::Sfixed64,
            ScalarType::Double,
            ScalarType::String,
            ScalarType::Bytes,
        ]
        .iter()
        .map(|t| format!("{:?} -> {}", t, t.proto_name()))
        .collect();

        assert_snapshot!(names.join("\n"), @r#"
        Int32 -> int32
        Int64 -> int64
        Uint32 -> uint32
        Uint64 -> uint64
        Sint32 -> sint32
        Sint64 -> sint64
        Bool -> bool
        Fixed32 -> fixed32
        Sfixed32 -> sfixed32
        Float -> float
        Fixed64 -> fixed64
        Sfixed64 -> sfixed64
        Double -> double
        String -> string
        Bytes -> bytes
        "#);
    }

    #[test]
    fn test_scalar_type_rust_types() {
        let types: Vec<_> = [
            ScalarType::Int32,
            ScalarType::Int64,
            ScalarType::Uint32,
            ScalarType::Uint64,
            ScalarType::Sint32,
            ScalarType::Sint64,
            ScalarType::Bool,
            ScalarType::Fixed32,
            ScalarType::Sfixed32,
            ScalarType::Float,
            ScalarType::Fixed64,
            ScalarType::Sfixed64,
            ScalarType::Double,
            ScalarType::String,
            ScalarType::Bytes,
        ]
        .iter()
        .map(|t| format!("{:?} -> {}", t, t.rust_type()))
        .collect();

        assert_snapshot!(types.join("\n"), @r#"
        Int32 -> i32
        Int64 -> i64
        Uint32 -> u32
        Uint64 -> u64
        Sint32 -> Sint32
        Sint64 -> Sint64
        Bool -> bool
        Fixed32 -> Fixed32
        Sfixed32 -> Sfixed32
        Float -> f32
        Fixed64 -> Fixed64
        Sfixed64 -> Sfixed64
        Double -> f64
        String -> ProtoString
        Bytes -> ProtoBytes
        "#);
    }

    #[test]
    fn test_field_name_generation() {
        let names: Vec<_> = [0u8, 1, 2, 25, 26, 27]
            .iter()
            .map(|&seed| format!("seed {} -> {}", seed, FieldDescriptor::generate_name(seed)))
            .collect();

        assert_snapshot!(names.join("\n"), @r#"
        seed 0 -> field_a
        seed 1 -> field_b
        seed 2 -> field_c
        seed 25 -> field_z
        seed 26 -> field_a
        seed 27 -> field_b
        "#);
    }

    #[test]
    fn test_message_name_generation() {
        let names: Vec<_> = [(0u8, 0usize), (1, 0), (0, 1), (1, 1), (0, 2), (0, 3)]
            .iter()
            .map(|&(seed, depth)| {
                format!(
                    "seed={}, depth={} -> {}",
                    seed,
                    depth,
                    MessageDescriptor::generate_name(seed, depth)
                )
            })
            .collect();

        assert_snapshot!(names.join("\n"), @r#"
        seed=0, depth=0 -> RootMessageA
        seed=1, depth=0 -> RootMessageB
        seed=0, depth=1 -> NestedMessageA
        seed=1, depth=1 -> NestedMessageB
        seed=0, depth=2 -> InnerMessageA
        seed=0, depth=3 -> DeepMessageA
        "#);
    }

    #[test]
    fn test_simple_schema_to_proto() {
        let schema = Schema {
            package: "test".to_string(),
            syntax: ProtobufSyntax::Proto3,
            messages: vec![MessageDescriptor {
                name: "TestMessage".to_string(),
                fields: vec![
                    FieldDescriptor {
                        name: "id".to_string(),
                        number: 1,
                        field_type: FieldType::Scalar(ScalarType::Int64),
                        cardinality: FieldCardinality::Singular,
                    },
                    FieldDescriptor {
                        name: "name".to_string(),
                        number: 2,
                        field_type: FieldType::Scalar(ScalarType::String),
                        cardinality: FieldCardinality::Optional,
                    },
                    FieldDescriptor {
                        name: "tags".to_string(),
                        number: 3,
                        field_type: FieldType::Scalar(ScalarType::String),
                        cardinality: FieldCardinality::Repeated,
                    },
                ],
                nested_messages: vec![],
            }],
        };

        assert_snapshot!(schema.to_proto(), @r#"
        syntax = "proto3";

        package test;

        message TestMessage {
          int64 id = 1;
          optional string name = 2;
          repeated string tags = 3;
        }

        "#);
    }

    #[test]
    fn test_nested_message_to_proto() {
        let schema = Schema {
            package: "test".to_string(),
            syntax: ProtobufSyntax::Proto3,
            messages: vec![MessageDescriptor {
                name: "Outer".to_string(),
                fields: vec![
                    FieldDescriptor {
                        name: "inner".to_string(),
                        number: 1,
                        field_type: FieldType::Message(0),
                        cardinality: FieldCardinality::Singular,
                    },
                    FieldDescriptor {
                        name: "count".to_string(),
                        number: 2,
                        field_type: FieldType::Scalar(ScalarType::Int32),
                        cardinality: FieldCardinality::Singular,
                    },
                ],
                nested_messages: vec![MessageDescriptor {
                    name: "Inner".to_string(),
                    fields: vec![
                        FieldDescriptor {
                            name: "value".to_string(),
                            number: 1,
                            field_type: FieldType::Scalar(ScalarType::Int32),
                            cardinality: FieldCardinality::Singular,
                        },
                        FieldDescriptor {
                            name: "label".to_string(),
                            number: 2,
                            field_type: FieldType::Scalar(ScalarType::String),
                            cardinality: FieldCardinality::Optional,
                        },
                    ],
                    nested_messages: vec![],
                }],
            }],
        };

        assert_snapshot!(schema.to_proto(), @r#"
        syntax = "proto3";

        package test;

        message Outer {
          message Inner {
            int32 value = 1;
            optional string label = 2;
          }
          Inner inner = 1;
          int32 count = 2;
        }

        "#);
    }

    #[test]
    fn test_deeply_nested_message() {
        let schema = Schema {
            package: "deep".to_string(),
            syntax: ProtobufSyntax::Proto2,
            messages: vec![MessageDescriptor {
                name: "Level0".to_string(),
                fields: vec![FieldDescriptor {
                    name: "next".to_string(),
                    number: 1,
                    field_type: FieldType::Message(0),
                    cardinality: FieldCardinality::Optional,
                }],
                nested_messages: vec![MessageDescriptor {
                    name: "Level1".to_string(),
                    fields: vec![FieldDescriptor {
                        name: "next".to_string(),
                        number: 1,
                        field_type: FieldType::Message(0),
                        cardinality: FieldCardinality::Optional,
                    }],
                    nested_messages: vec![MessageDescriptor {
                        name: "Level2".to_string(),
                        fields: vec![FieldDescriptor {
                            name: "data".to_string(),
                            number: 1,
                            field_type: FieldType::Scalar(ScalarType::Bytes),
                            cardinality: FieldCardinality::Singular,
                        }],
                        nested_messages: vec![],
                    }],
                }],
            }],
        };

        assert_snapshot!(schema.to_proto(), @r#"
        syntax = "proto2";

        package deep;

        message Level0 {
          message Level1 {
            message Level2 {
              optional bytes data = 1;
            }
            optional Level2 next = 1;
          }
          optional Level1 next = 1;
        }
        "#);
    }

    #[test]
    fn test_arbitrary_schema_generation() {
        // Use a fixed seed for deterministic output
        let data: &[u8] = &[
            0x42, 0x13, 0x37, 0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01, 0x02, 0x03, 0x04, 0x05,
            0x06, 0x07, 0x08, 0x09,
        ];
        let mut u = Unstructured::new(data);

        let schema = Schema::arbitrary(&mut u).expect("should generate schema");

        // Verify basic constraints
        assert!(!schema.messages.is_empty());
        for msg in &schema.messages {
            assert!(!msg.fields.is_empty());
            // Verify field numbers are unique within message
            let numbers: HashSet<_> = msg.fields.iter().map(|f| f.number).collect();
            assert_eq!(numbers.len(), msg.fields.len());
        }

        assert_snapshot!(schema.to_proto(), @r#"
        syntax = "proto3";

        package fuzz.test;

        message RootMessageA {
          message NestedMessageB {
            message InnerMessageC {
              repeated sfixed32 field_a = 899;
              int32 field_b = 487;
            }
            message InnerMessageD {
              int32 field_a = 1;
            }
            InnerMessageC field_a = 1;
          }
          NestedMessageB field_a = 1;
        }

        message RootMessageB {
          int32 field_a = 1;
        }

        message RootMessageC {
          int32 field_a = 1;
        }

        "#);
    }

    #[test]
    fn test_schema_counts() {
        let schema = Schema {
            package: "test".to_string(),
            syntax: ProtobufSyntax::Proto3,
            messages: vec![MessageDescriptor {
                name: "Root".to_string(),
                fields: vec![
                    FieldDescriptor {
                        name: "a".to_string(),
                        number: 1,
                        field_type: FieldType::Scalar(ScalarType::Int32),
                        cardinality: FieldCardinality::Singular,
                    },
                    FieldDescriptor {
                        name: "b".to_string(),
                        number: 2,
                        field_type: FieldType::Message(0),
                        cardinality: FieldCardinality::Singular,
                    },
                ],
                nested_messages: vec![MessageDescriptor {
                    name: "Nested".to_string(),
                    fields: vec![FieldDescriptor {
                        name: "c".to_string(),
                        number: 1,
                        field_type: FieldType::Scalar(ScalarType::Bool),
                        cardinality: FieldCardinality::Singular,
                    }],
                    nested_messages: vec![],
                }],
            }],
        };

        assert_snapshot!(format!(
            "messages: {}, fields: {}",
            schema.total_message_count(),
            schema.total_field_count()
        ), @"messages: 2, fields: 3");
    }

    #[test]
    fn test_field_number_avoids_reserved() {
        let mut used = HashSet::new();
        // Fill up numbers before reserved range
        for i in 1..19000 {
            used.insert(i);
        }

        let num = FieldDescriptor::generate_field_number(18999, &used);
        // Should skip the reserved range 19000-19999
        assert!(!(19000..20000).contains(&num));
        assert_snapshot!(format!("generated field number: {}", num), @"generated field number: 20000");
    }

    #[test]
    fn test_all_scalar_types_schema() {
        let fields: Vec<_> = [
            ScalarType::Int32,
            ScalarType::Int64,
            ScalarType::Uint32,
            ScalarType::Uint64,
            ScalarType::Sint32,
            ScalarType::Sint64,
            ScalarType::Bool,
            ScalarType::Fixed32,
            ScalarType::Sfixed32,
            ScalarType::Float,
            ScalarType::Fixed64,
            ScalarType::Sfixed64,
            ScalarType::Double,
            ScalarType::String,
            ScalarType::Bytes,
        ]
        .iter()
        .enumerate()
        .map(|(i, &scalar)| FieldDescriptor {
            name: format!("{}_field", scalar.proto_name()),
            number: (i + 1) as u32,
            field_type: FieldType::Scalar(scalar),
            cardinality: FieldCardinality::Singular,
        })
        .collect();

        let schema = Schema {
            package: "scalars".to_string(),
            syntax: ProtobufSyntax::Proto3,
            messages: vec![MessageDescriptor {
                name: "AllScalars".to_string(),
                fields,
                nested_messages: vec![],
            }],
        };

        assert_snapshot!(schema.to_proto(), @r#"
        syntax = "proto3";

        package scalars;

        message AllScalars {
          int32 int32_field = 1;
          int64 int64_field = 2;
          uint32 uint32_field = 3;
          uint64 uint64_field = 4;
          sint32 sint32_field = 5;
          sint64 sint64_field = 6;
          bool bool_field = 7;
          fixed32 fixed32_field = 8;
          sfixed32 sfixed32_field = 9;
          float float_field = 10;
          fixed64 fixed64_field = 11;
          sfixed64 sfixed64_field = 12;
          double double_field = 13;
          string string_field = 14;
          bytes bytes_field = 15;
        }

        "#);
    }
}
