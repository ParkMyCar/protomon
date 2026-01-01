//! Random value generation for protobuf schemas.
//!
//! This module provides types and functions to generate random values
//! that conform to a given protobuf schema, and serialize them to JSON
//! for cross-language testing.

use crate::{FieldCardinality, FieldDescriptor, FieldType, MessageDescriptor, ScalarType, Schema};
use arbitrary::{Arbitrary, Unstructured};
use std::collections::BTreeMap;

/// Maximum number of elements in a repeated field.
const MAX_REPEATED_ELEMENTS: usize = 8;

/// Maximum length for string values.
const MAX_STRING_LEN: usize = 64;

/// Maximum length for bytes values.
const MAX_BYTES_LEN: usize = 64;

/// A concrete scalar value.
#[derive(Debug, Clone, PartialEq)]
pub enum ScalarValue {
    Int32(i32),
    Int64(i64),
    Uint32(u32),
    Uint64(u64),
    Sint32(i32),
    Sint64(i64),
    Bool(bool),
    Fixed32(u32),
    Sfixed32(i32),
    Float(f32),
    Fixed64(u64),
    Sfixed64(i64),
    Double(f64),
    String(String),
    Bytes(Vec<u8>),
}

impl ScalarValue {
    /// Generate a random scalar value of the given type.
    pub fn arbitrary(scalar_type: ScalarType, u: &mut Unstructured<'_>) -> arbitrary::Result<Self> {
        Ok(match scalar_type {
            ScalarType::Int32 => ScalarValue::Int32(u.arbitrary()?),
            ScalarType::Int64 => ScalarValue::Int64(u.arbitrary()?),
            ScalarType::Uint32 => ScalarValue::Uint32(u.arbitrary()?),
            ScalarType::Uint64 => ScalarValue::Uint64(u.arbitrary()?),
            ScalarType::Sint32 => ScalarValue::Sint32(u.arbitrary()?),
            ScalarType::Sint64 => ScalarValue::Sint64(u.arbitrary()?),
            ScalarType::Bool => ScalarValue::Bool(u.arbitrary()?),
            ScalarType::Fixed32 => ScalarValue::Fixed32(u.arbitrary()?),
            ScalarType::Sfixed32 => ScalarValue::Sfixed32(u.arbitrary()?),
            ScalarType::Float => {
                // Generate finite floats to avoid JSON serialization issues
                let f: f32 = u.arbitrary()?;
                ScalarValue::Float(if f.is_finite() { f } else { 0.0 })
            }
            ScalarType::Fixed64 => ScalarValue::Fixed64(u.arbitrary()?),
            ScalarType::Sfixed64 => ScalarValue::Sfixed64(u.arbitrary()?),
            ScalarType::Double => {
                // Generate finite doubles to avoid JSON serialization issues
                let f: f64 = u.arbitrary()?;
                ScalarValue::Double(if f.is_finite() { f } else { 0.0 })
            }
            ScalarType::String => {
                let len = u.int_in_range(0..=MAX_STRING_LEN)?;
                let s: String = (0..len)
                    .map(|_| {
                        // Generate printable ASCII for readability
                        let c = u.int_in_range(0x20u8..=0x7E).unwrap_or(b'?');
                        c as char
                    })
                    .collect();
                ScalarValue::String(s)
            }
            ScalarType::Bytes => {
                let len = u.int_in_range(0..=MAX_BYTES_LEN)?;
                let bytes: Vec<u8> = (0..len).map(|_| u.arbitrary().unwrap_or(0)).collect();
                ScalarValue::Bytes(bytes)
            }
        })
    }

    /// Convert to JSON value.
    pub fn to_json(&self) -> String {
        match self {
            ScalarValue::Int32(v) => v.to_string(),
            ScalarValue::Int64(v) => format!("\"{}\"", v), // JSON uses string for 64-bit
            ScalarValue::Uint32(v) => v.to_string(),
            ScalarValue::Uint64(v) => format!("\"{}\"", v), // JSON uses string for 64-bit
            ScalarValue::Sint32(v) => v.to_string(),
            ScalarValue::Sint64(v) => format!("\"{}\"", v), // JSON uses string for 64-bit
            ScalarValue::Bool(v) => v.to_string(),
            ScalarValue::Fixed32(v) => v.to_string(),
            ScalarValue::Sfixed32(v) => v.to_string(),
            ScalarValue::Float(v) => {
                if *v == 0.0 && v.is_sign_negative() {
                    "0".to_string() // Normalize -0.0 to 0
                } else {
                    format!("{}", v)
                }
            }
            ScalarValue::Fixed64(v) => format!("\"{}\"", v), // JSON uses string for 64-bit
            ScalarValue::Sfixed64(v) => format!("\"{}\"", v), // JSON uses string for 64-bit
            ScalarValue::Double(v) => {
                if *v == 0.0 && v.is_sign_negative() {
                    "0".to_string() // Normalize -0.0 to 0
                } else {
                    format!("{}", v)
                }
            }
            ScalarValue::String(v) => format!("\"{}\"", escape_json_string(v)),
            ScalarValue::Bytes(v) => {
                // Protobuf JSON uses base64 for bytes
                use base64::{engine::general_purpose::STANDARD, Engine};
                format!("\"{}\"", STANDARD.encode(v))
            }
        }
    }
}

/// A field value - can be singular, optional (present or absent), or repeated.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// A singular scalar value.
    Scalar(ScalarValue),
    /// A nested message value.
    Message(Box<MessageValue>),
    /// A repeated field with multiple values.
    Repeated(Vec<FieldValue>),
    /// An absent optional field.
    Absent,
}

impl FieldValue {
    /// Returns true if this is an absent optional field.
    pub fn is_absent(&self) -> bool {
        matches!(self, FieldValue::Absent)
    }
}

/// A message value containing field values keyed by field name.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageValue {
    /// Field values keyed by field name.
    pub fields: BTreeMap<String, FieldValue>,
}

impl MessageValue {
    /// Create an empty message value.
    pub fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Generate a random message value conforming to the given descriptor.
    pub fn arbitrary(
        descriptor: &MessageDescriptor,
        u: &mut Unstructured<'_>,
    ) -> arbitrary::Result<Self> {
        let mut fields = BTreeMap::new();

        for field in &descriptor.fields {
            let value = generate_field_value(field, &descriptor.nested_messages, u)?;
            if !value.is_absent() {
                fields.insert(field.name.clone(), value);
            }
        }

        Ok(Self { fields })
    }

    /// Convert to JSON string.
    pub fn to_json(&self) -> String {
        let mut parts = Vec::new();

        for (name, value) in &self.fields {
            if let Some(json_value) = field_value_to_json(value) {
                parts.push(format!("\"{}\": {}", name, json_value));
            }
        }

        format!("{{{}}}", parts.join(", "))
    }

    /// Convert to pretty-printed JSON string.
    pub fn to_json_pretty(&self) -> String {
        to_json_pretty_inner(self, 0)
    }

    /// Convert to protobuf text format (for use with `protoc --encode`).
    pub fn to_text_format(&self) -> String {
        to_text_format_inner(self, 0)
    }
}

impl Default for MessageValue {
    fn default() -> Self {
        Self::new()
    }
}

/// A complete test case with schema and values.
#[derive(Debug, Clone)]
pub struct TestCase {
    /// The protobuf schema.
    pub schema: Schema,
    /// Values for each top-level message in the schema.
    pub values: Vec<(String, MessageValue)>,
}

impl TestCase {
    /// Generate a random test case.
    pub fn arbitrary(u: &mut Unstructured<'_>) -> arbitrary::Result<Self> {
        let schema = Schema::arbitrary(u)?;
        let mut values = Vec::new();

        for message in &schema.messages {
            let value = MessageValue::arbitrary(message, u)?;
            values.push((message.name.clone(), value));
        }

        Ok(Self { schema, values })
    }

    /// Output the .proto file content.
    pub fn to_proto(&self) -> String {
        self.schema.to_proto()
    }

    /// Output JSON for each message value.
    pub fn to_json_files(&self) -> Vec<(String, String)> {
        self.values
            .iter()
            .map(|(name, value)| (format!("{}.json", name), value.to_json_pretty()))
            .collect()
    }
}

// Helper functions

fn generate_field_value(
    field: &FieldDescriptor,
    nested_messages: &[MessageDescriptor],
    u: &mut Unstructured<'_>,
) -> arbitrary::Result<FieldValue> {
    match field.cardinality {
        FieldCardinality::Singular => generate_singular_value(field, nested_messages, u),
        FieldCardinality::Optional => {
            // 50% chance of being present
            if u.ratio(1, 2)? {
                generate_singular_value(field, nested_messages, u)
            } else {
                Ok(FieldValue::Absent)
            }
        }
        FieldCardinality::Repeated => {
            let count = u.int_in_range(0..=MAX_REPEATED_ELEMENTS)?;
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(generate_singular_value(field, nested_messages, u)?);
            }
            Ok(FieldValue::Repeated(values))
        }
    }
}

fn generate_singular_value(
    field: &FieldDescriptor,
    nested_messages: &[MessageDescriptor],
    u: &mut Unstructured<'_>,
) -> arbitrary::Result<FieldValue> {
    match &field.field_type {
        FieldType::Scalar(scalar_type) => {
            Ok(FieldValue::Scalar(ScalarValue::arbitrary(*scalar_type, u)?))
        }
        FieldType::Message(idx) => {
            if *idx < nested_messages.len() {
                let nested_value = MessageValue::arbitrary(&nested_messages[*idx], u)?;
                Ok(FieldValue::Message(Box::new(nested_value)))
            } else {
                // Invalid reference, generate empty message
                Ok(FieldValue::Message(Box::default()))
            }
        }
    }
}

fn field_value_to_json(value: &FieldValue) -> Option<String> {
    match value {
        FieldValue::Scalar(scalar) => Some(scalar.to_json()),
        FieldValue::Message(msg) => Some(msg.to_json()),
        FieldValue::Repeated(values) => {
            let items: Vec<String> = values.iter().filter_map(field_value_to_json).collect();
            Some(format!("[{}]", items.join(", ")))
        }
        FieldValue::Absent => None,
    }
}

fn to_json_pretty_inner(msg: &MessageValue, indent: usize) -> String {
    if msg.fields.is_empty() {
        return "{}".to_string();
    }

    let indent_str = "  ".repeat(indent + 1);
    let close_indent = "  ".repeat(indent);
    let mut parts = Vec::new();

    for (name, value) in &msg.fields {
        if let Some(json_value) = field_value_to_json_pretty(value, indent + 1) {
            parts.push(format!("{}\"{}\": {}", indent_str, name, json_value));
        }
    }

    format!("{{\n{}\n{}}}", parts.join(",\n"), close_indent)
}

fn field_value_to_json_pretty(value: &FieldValue, indent: usize) -> Option<String> {
    match value {
        FieldValue::Scalar(scalar) => Some(scalar.to_json()),
        FieldValue::Message(msg) => Some(to_json_pretty_inner(msg, indent)),
        FieldValue::Repeated(values) => {
            if values.is_empty() {
                return Some("[]".to_string());
            }
            let indent_str = "  ".repeat(indent + 1);
            let close_indent = "  ".repeat(indent);
            let items: Vec<String> = values
                .iter()
                .filter_map(|v| field_value_to_json_pretty(v, indent + 1))
                .map(|s| format!("{}{}", indent_str, s))
                .collect();
            Some(format!("[\n{}\n{}]", items.join(",\n"), close_indent))
        }
        FieldValue::Absent => None,
    }
}

fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

// Text format helpers (for protoc --encode)

fn to_text_format_inner(msg: &MessageValue, indent: usize) -> String {
    let mut lines = Vec::new();
    let indent_str = "  ".repeat(indent);

    for (name, value) in &msg.fields {
        match value {
            FieldValue::Scalar(scalar) => {
                lines.push(format!(
                    "{}{}: {}",
                    indent_str,
                    name,
                    scalar_to_text(scalar)
                ));
            }
            FieldValue::Message(nested) => {
                lines.push(format!("{}{} {{", indent_str, name));
                lines.push(to_text_format_inner(nested, indent + 1));
                lines.push(format!("{}}}", indent_str));
            }
            FieldValue::Repeated(values) => {
                for v in values {
                    match v {
                        FieldValue::Scalar(scalar) => {
                            lines.push(format!(
                                "{}{}: {}",
                                indent_str,
                                name,
                                scalar_to_text(scalar)
                            ));
                        }
                        FieldValue::Message(nested) => {
                            lines.push(format!("{}{} {{", indent_str, name));
                            lines.push(to_text_format_inner(nested, indent + 1));
                            lines.push(format!("{}}}", indent_str));
                        }
                        _ => {}
                    }
                }
            }
            FieldValue::Absent => {}
        }
    }

    lines.join("\n")
}

fn scalar_to_text(scalar: &ScalarValue) -> String {
    match scalar {
        ScalarValue::Int32(v) => v.to_string(),
        ScalarValue::Int64(v) => v.to_string(),
        ScalarValue::Uint32(v) => v.to_string(),
        ScalarValue::Uint64(v) => v.to_string(),
        ScalarValue::Sint32(v) => v.to_string(),
        ScalarValue::Sint64(v) => v.to_string(),
        ScalarValue::Bool(v) => v.to_string(),
        ScalarValue::Fixed32(v) => v.to_string(),
        ScalarValue::Sfixed32(v) => v.to_string(),
        ScalarValue::Float(v) => {
            if v.is_nan() {
                "nan".to_string()
            } else if v.is_infinite() {
                if *v > 0.0 {
                    "inf".to_string()
                } else {
                    "-inf".to_string()
                }
            } else {
                format!("{}", v)
            }
        }
        ScalarValue::Fixed64(v) => v.to_string(),
        ScalarValue::Sfixed64(v) => v.to_string(),
        ScalarValue::Double(v) => {
            if v.is_nan() {
                "nan".to_string()
            } else if v.is_infinite() {
                if *v > 0.0 {
                    "inf".to_string()
                } else {
                    "-inf".to_string()
                }
            } else {
                format!("{}", v)
            }
        }
        ScalarValue::String(v) => format!("\"{}\"", escape_text_string(v)),
        ScalarValue::Bytes(v) => format!("\"{}\"", escape_bytes_for_text(v)),
    }
}

fn escape_text_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                // Use octal escape for control characters
                result.push_str(&format!("\\{:03o}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

fn escape_bytes_for_text(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 4);
    for &b in bytes {
        match b {
            b'"' => result.push_str("\\\""),
            b'\\' => result.push_str("\\\\"),
            b'\n' => result.push_str("\\n"),
            b'\r' => result.push_str("\\r"),
            b'\t' => result.push_str("\\t"),
            0x20..=0x7e => result.push(b as char),
            _ => result.push_str(&format!("\\{:03o}", b)),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtobufSyntax;
    use insta::assert_snapshot;

    #[test]
    fn test_scalar_value_json() {
        let values = vec![
            ("int32", ScalarValue::Int32(-42)),
            ("int64", ScalarValue::Int64(i64::MAX)),
            ("uint32", ScalarValue::Uint32(42)),
            ("uint64", ScalarValue::Uint64(u64::MAX)),
            ("bool_true", ScalarValue::Bool(true)),
            ("bool_false", ScalarValue::Bool(false)),
            ("float", ScalarValue::Float(3.125)),
            ("double", ScalarValue::Double(2.75)),
            ("string", ScalarValue::String("hello world".to_string())),
            (
                "string_escape",
                ScalarValue::String("line1\nline2\ttab\"quote".to_string()),
            ),
            ("bytes", ScalarValue::Bytes(vec![0, 1, 2, 255])),
        ];

        let json_output: Vec<String> = values
            .iter()
            .map(|(name, v)| format!("{}: {}", name, v.to_json()))
            .collect();

        assert_snapshot!(json_output.join("\n"), @r#"
        int32: -42
        int64: "9223372036854775807"
        uint32: 42
        uint64: "18446744073709551615"
        bool_true: true
        bool_false: false
        float: 3.125
        double: 2.75
        string: "hello world"
        string_escape: "line1\nline2\ttab\"quote"
        bytes: "AAEC/w=="
        "#);
    }

    #[test]
    fn test_simple_message_json() {
        let mut msg = MessageValue::new();
        msg.fields.insert(
            "id".to_string(),
            FieldValue::Scalar(ScalarValue::Int32(123)),
        );
        msg.fields.insert(
            "name".to_string(),
            FieldValue::Scalar(ScalarValue::String("test".to_string())),
        );
        msg.fields.insert(
            "active".to_string(),
            FieldValue::Scalar(ScalarValue::Bool(true)),
        );

        assert_snapshot!(msg.to_json_pretty(), @r#"
        {
          "active": true,
          "id": 123,
          "name": "test"
        }
        "#);
    }

    #[test]
    fn test_nested_message_json() {
        let mut inner = MessageValue::new();
        inner.fields.insert(
            "value".to_string(),
            FieldValue::Scalar(ScalarValue::Int32(42)),
        );

        let mut outer = MessageValue::new();
        outer
            .fields
            .insert("nested".to_string(), FieldValue::Message(Box::new(inner)));
        outer.fields.insert(
            "label".to_string(),
            FieldValue::Scalar(ScalarValue::String("outer".to_string())),
        );

        assert_snapshot!(outer.to_json_pretty(), @r#"
        {
          "label": "outer",
          "nested": {
            "value": 42
          }
        }
        "#);
    }

    #[test]
    fn test_repeated_field_json() {
        let mut msg = MessageValue::new();
        msg.fields.insert(
            "numbers".to_string(),
            FieldValue::Repeated(vec![
                FieldValue::Scalar(ScalarValue::Int32(1)),
                FieldValue::Scalar(ScalarValue::Int32(2)),
                FieldValue::Scalar(ScalarValue::Int32(3)),
            ]),
        );
        msg.fields.insert(
            "tags".to_string(),
            FieldValue::Repeated(vec![
                FieldValue::Scalar(ScalarValue::String("a".to_string())),
                FieldValue::Scalar(ScalarValue::String("b".to_string())),
            ]),
        );

        assert_snapshot!(msg.to_json_pretty(), @r#"
        {
          "numbers": [
            1,
            2,
            3
          ],
          "tags": [
            "a",
            "b"
          ]
        }
        "#);
    }

    #[test]
    fn test_text_format() {
        let mut msg = MessageValue::new();
        msg.fields
            .insert("id".to_string(), FieldValue::Scalar(ScalarValue::Int32(42)));
        msg.fields.insert(
            "name".to_string(),
            FieldValue::Scalar(ScalarValue::String("Hello, World!".to_string())),
        );
        msg.fields.insert(
            "score".to_string(),
            FieldValue::Scalar(ScalarValue::Double(98.6)),
        );
        msg.fields.insert(
            "active".to_string(),
            FieldValue::Scalar(ScalarValue::Bool(true)),
        );
        msg.fields.insert(
            "tags".to_string(),
            FieldValue::Repeated(vec![
                FieldValue::Scalar(ScalarValue::String("rust".to_string())),
                FieldValue::Scalar(ScalarValue::String("protobuf".to_string())),
            ]),
        );
        msg.fields.insert(
            "data".to_string(),
            FieldValue::Scalar(ScalarValue::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF])),
        );

        assert_snapshot!(msg.to_text_format(), @r#"
        active: true
        data: "\336\255\276\357"
        id: 42
        name: "Hello, World!"
        score: 98.6
        tags: "rust"
        tags: "protobuf"
        "#);
    }

    #[test]
    fn test_text_format_nested() {
        let mut inner = MessageValue::new();
        inner.fields.insert(
            "value".to_string(),
            FieldValue::Scalar(ScalarValue::Int32(42)),
        );
        inner.fields.insert(
            "label".to_string(),
            FieldValue::Scalar(ScalarValue::String("nested".to_string())),
        );

        let mut outer = MessageValue::new();
        outer
            .fields
            .insert("id".to_string(), FieldValue::Scalar(ScalarValue::Int32(1)));
        outer
            .fields
            .insert("nested".to_string(), FieldValue::Message(Box::new(inner)));

        assert_snapshot!(outer.to_text_format(), @r#"
        id: 1
        nested {
          label: "nested"
          value: 42
        }
        "#);
    }

    #[test]
    fn test_arbitrary_test_case() {
        let data: &[u8] = &[
            0x42, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
            0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A,
            0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26,
        ];
        let mut u = Unstructured::new(data);

        let test_case = TestCase::arbitrary(&mut u).expect("should generate test case");

        assert_snapshot!(test_case.to_proto(), @r#"
        syntax = "proto2";

        package fuzz.test;

        message RootMessageA {
          message NestedMessageB {
            message InnerMessageC {
              optional int32 field_a = 49;
              optional int64 field_b = 157;
              optional int32 field_c = 265;
              optional int32 field_d = 1;
              optional int32 field_e = 2;
              optional int32 field_f = 3;
              optional int32 field_g = 4;
              optional int32 field_h = 5;
            }
            message InnerMessageD {
              optional int32 field_a = 1;
            }
            optional InnerMessageC field_a = 1;
          }
          optional NestedMessageB field_a = 1;
        }
        "#);

        // Collect JSON outputs
        let json_files = test_case.to_json_files();
        assert_eq!(json_files.len(), 1);
        let (name, json) = &json_files[0];
        assert_eq!(name, "RootMessageA.json");
        assert_snapshot!(json, @r#"
        {
          "field_a": {
            "field_a": {
              "field_a": 0,
              "field_b": "0",
              "field_c": 0,
              "field_d": 0,
              "field_e": 0,
              "field_f": 0,
              "field_g": 0,
              "field_h": 0
            }
          }
        }
        "#);
    }

    #[test]
    fn test_full_example() {
        // Manually construct a schema and values to show the full flow
        let schema = Schema {
            package: "example".to_string(),
            syntax: ProtobufSyntax::Proto3,
            messages: vec![MessageDescriptor {
                name: "Person".to_string(),
                fields: vec![
                    FieldDescriptor {
                        name: "id".to_string(),
                        number: 1,
                        field_type: FieldType::Scalar(ScalarType::Int32),
                        cardinality: FieldCardinality::Singular,
                    },
                    FieldDescriptor {
                        name: "name".to_string(),
                        number: 2,
                        field_type: FieldType::Scalar(ScalarType::String),
                        cardinality: FieldCardinality::Singular,
                    },
                    FieldDescriptor {
                        name: "email".to_string(),
                        number: 3,
                        field_type: FieldType::Scalar(ScalarType::String),
                        cardinality: FieldCardinality::Optional,
                    },
                    FieldDescriptor {
                        name: "phone_numbers".to_string(),
                        number: 4,
                        field_type: FieldType::Scalar(ScalarType::String),
                        cardinality: FieldCardinality::Repeated,
                    },
                ],
                nested_messages: vec![],
            }],
        };

        let mut person = MessageValue::new();
        person.fields.insert(
            "id".to_string(),
            FieldValue::Scalar(ScalarValue::Int32(123)),
        );
        person.fields.insert(
            "name".to_string(),
            FieldValue::Scalar(ScalarValue::String("Alice".to_string())),
        );
        person.fields.insert(
            "email".to_string(),
            FieldValue::Scalar(ScalarValue::String("alice@example.com".to_string())),
        );
        person.fields.insert(
            "phone_numbers".to_string(),
            FieldValue::Repeated(vec![
                FieldValue::Scalar(ScalarValue::String("+1-555-1234".to_string())),
                FieldValue::Scalar(ScalarValue::String("+1-555-5678".to_string())),
            ]),
        );

        assert_snapshot!(schema.to_proto(), @r#"
        syntax = "proto3";

        package example;

        message Person {
          int32 id = 1;
          string name = 2;
          optional string email = 3;
          repeated string phone_numbers = 4;
        }

        "#);

        assert_snapshot!(person.to_json_pretty(), @r#"
        {
          "email": "alice@example.com",
          "id": 123,
          "name": "Alice",
          "phone_numbers": [
            "+1-555-1234",
            "+1-555-5678"
          ]
        }
        "#);
    }
}
