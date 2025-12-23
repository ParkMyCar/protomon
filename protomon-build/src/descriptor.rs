//! Descriptor types for protobuf FileDescriptorSet.
//!
//! These types mirror google/protobuf/descriptor.proto but are implemented
//! independently to avoid depending on prost.

use crate::Error;
use protomon::codec::ProtoMessage;
use protomon::ProtoMessage as ProtoMessageDerive;

/// Decode a FileDescriptorSet from protobuf binary data.
pub fn decode_file_descriptor_set(data: &[u8]) -> Result<FileDescriptorSet, Error> {
    FileDescriptorSet::decode_message(bytes::Bytes::from(data.to_vec()))
        .map_err(|e| Error::DecodeError(e.to_string()))
}

/// A collection of file descriptors.
/// Corresponds to google.protobuf.FileDescriptorSet.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct FileDescriptorSet {
    /// The file descriptors.
    #[proto(tag = 1, repeated)]
    pub file: Vec<FileDescriptorProto>,
}

/// Describes a complete .proto file.
/// Corresponds to google.protobuf.FileDescriptorProto.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct FileDescriptorProto {
    /// The file name, relative to root of source tree.
    #[proto(tag = 1, optional)]
    pub name: Option<String>,
    /// The package name.
    #[proto(tag = 2, optional)]
    pub package: Option<String>,
    /// Names of files imported by this file.
    #[proto(tag = 3, repeated)]
    pub dependency: Vec<String>,
    /// All top-level message definitions in this file.
    #[proto(tag = 4, repeated)]
    pub message_type: Vec<DescriptorProto>,
    /// All top-level enum definitions in this file.
    #[proto(tag = 5, repeated)]
    pub enum_type: Vec<EnumDescriptorProto>,
    /// The syntax of the proto file (e.g., "proto2", "proto3").
    #[proto(tag = 12, optional)]
    pub syntax: Option<String>,
}

/// Describes a message type.
/// Corresponds to google.protobuf.DescriptorProto.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct DescriptorProto {
    /// The message name.
    #[proto(tag = 1, optional)]
    pub name: Option<String>,
    /// Fields of the message.
    #[proto(tag = 2, repeated)]
    pub field: Vec<FieldDescriptorProto>,
    /// Nested message types.
    #[proto(tag = 3, repeated)]
    pub nested_type: Vec<DescriptorProto>,
    /// Nested enum types.
    #[proto(tag = 4, repeated)]
    pub enum_type: Vec<EnumDescriptorProto>,
    /// Message options.
    #[proto(tag = 7, optional)]
    pub options: Option<MessageOptions>,
    /// Oneof declarations.
    #[proto(tag = 8, repeated)]
    pub oneof_decl: Vec<OneofDescriptorProto>,
}

/// Describes a field within a message.
/// Corresponds to google.protobuf.FieldDescriptorProto.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct FieldDescriptorProto {
    /// The field name.
    #[proto(tag = 1, optional)]
    pub name: Option<String>,
    /// The field number (tag).
    #[proto(tag = 3, optional)]
    pub number: Option<i32>,
    /// The field label (optional, required, repeated).
    #[proto(tag = 4, optional)]
    pub label: Option<i32>,
    /// The field type.
    #[proto(tag = 5, optional)]
    pub r#type: Option<i32>,
    /// For message and enum types, the fully-qualified type name.
    #[proto(tag = 6, optional)]
    pub type_name: Option<String>,
    /// The default value as a string.
    #[proto(tag = 7, optional)]
    pub default_value: Option<String>,
    /// Field options (includes protomon extensions).
    #[proto(tag = 8, optional)]
    pub options: Option<FieldOptions>,
    /// If set, this field is part of a oneof.
    #[proto(tag = 9, optional)]
    pub oneof_index: Option<i32>,
    /// The JSON name for this field.
    #[proto(tag = 10, optional)]
    pub json_name: Option<String>,
    /// If true, this is a proto3 optional field.
    #[proto(tag = 17, optional)]
    pub proto3_optional: Option<bool>,
}

impl FieldDescriptorProto {
    /// Get the field label.
    pub fn label(&self) -> Label {
        self.label.and_then(Label::from_i32).unwrap_or(Label::Optional)
    }

    /// Get the field type.
    pub fn field_type(&self) -> Option<Type> {
        self.r#type.and_then(Type::from_i32)
    }
}

/// Describes an enum type.
/// Corresponds to google.protobuf.EnumDescriptorProto.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct EnumDescriptorProto {
    /// The enum name.
    #[proto(tag = 1, optional)]
    pub name: Option<String>,
    /// The enum values.
    #[proto(tag = 2, repeated)]
    pub value: Vec<EnumValueDescriptorProto>,
}

/// Describes an enum value.
/// Corresponds to google.protobuf.EnumValueDescriptorProto.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct EnumValueDescriptorProto {
    /// The value name.
    #[proto(tag = 1, optional)]
    pub name: Option<String>,
    /// The value number.
    #[proto(tag = 2, optional)]
    pub number: Option<i32>,
}

/// Describes a oneof.
/// Corresponds to google.protobuf.OneofDescriptorProto.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct OneofDescriptorProto {
    /// The oneof name.
    #[proto(tag = 1, optional)]
    pub name: Option<String>,
    /// Oneof options (includes protomon extensions).
    #[proto(tag = 2, optional)]
    pub options: Option<OneofOptions>,
}

/// Options for a oneof.
/// Corresponds to google.protobuf.OneofOptions with protomon extensions.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct OneofOptions {
    // Protomon extensions (field numbers 50000-50049 reserved for oneof options)

    /// Whether the oneof is nullable (wrapped in `Option<T>`).
    /// Default is true. When false, decoding fails if oneof is missing.
    /// Extension field 50000.
    #[proto(tag = 50000, optional)]
    pub nullable: Option<bool>,
}

impl OneofOptions {
    /// Returns whether this oneof is nullable.
    /// Defaults to true if not explicitly set.
    pub fn is_nullable(&self) -> bool {
        self.nullable.unwrap_or(true)
    }
}

/// Options for a message type.
/// Corresponds to google.protobuf.MessageOptions.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct MessageOptions {
    /// Set true if this message is a map entry type.
    #[proto(tag = 7, optional)]
    pub map_entry: Option<bool>,
}

/// Options for a field.
/// Corresponds to google.protobuf.FieldOptions with protomon extensions.
#[derive(Debug, Clone, Default, ProtoMessageDerive)]
pub struct FieldOptions {
    // Protomon extensions (field numbers 50001-50099)

    /// Use `Vec<T>` instead of `Repeated<T>` for repeated fields.
    /// Extension field 50001.
    #[proto(tag = 50001)]
    pub vec: bool,

    /// Wrap field type in `Box<T>`.
    /// Extension field 50002.
    #[proto(tag = 50002)]
    pub boxed: bool,

    /// Wrap message field in `LazyMessage<T>` for lazy/zero-copy decoding.
    /// Extension field 50003.
    #[proto(tag = 50003)]
    pub lazy: bool,

    /// Use fixed-size array `[u8; N]` instead of `ProtoBytes` for bytes fields.
    /// Extension field 50004. Value of 0 means not set.
    #[proto(tag = 50004)]
    pub fixed_array: u32,
}

/// Field type enumeration.
/// Corresponds to google.protobuf.FieldDescriptorProto.Type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Type {
    Double = 1,
    Float = 2,
    Int64 = 3,
    Uint64 = 4,
    Int32 = 5,
    Fixed64 = 6,
    Fixed32 = 7,
    Bool = 8,
    String = 9,
    Group = 10,
    Message = 11,
    Bytes = 12,
    Uint32 = 13,
    Enum = 14,
    Sfixed32 = 15,
    Sfixed64 = 16,
    Sint32 = 17,
    Sint64 = 18,
}

impl Type {
    /// Convert from i32.
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Double),
            2 => Some(Self::Float),
            3 => Some(Self::Int64),
            4 => Some(Self::Uint64),
            5 => Some(Self::Int32),
            6 => Some(Self::Fixed64),
            7 => Some(Self::Fixed32),
            8 => Some(Self::Bool),
            9 => Some(Self::String),
            10 => Some(Self::Group),
            11 => Some(Self::Message),
            12 => Some(Self::Bytes),
            13 => Some(Self::Uint32),
            14 => Some(Self::Enum),
            15 => Some(Self::Sfixed32),
            16 => Some(Self::Sfixed64),
            17 => Some(Self::Sint32),
            18 => Some(Self::Sint64),
            _ => None,
        }
    }
}

/// Field label enumeration.
/// Corresponds to google.protobuf.FieldDescriptorProto.Label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Label {
    Optional = 1,
    Required = 2,
    Repeated = 3,
}

impl Label {
    /// Convert from i32.
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Optional),
            2 => Some(Self::Required),
            3 => Some(Self::Repeated),
            _ => None,
        }
    }
}
