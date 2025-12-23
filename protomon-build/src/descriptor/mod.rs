//! Descriptor types for protobuf FileDescriptorSet.
//!
//! These types mirror google/protobuf/descriptor.proto but are implemented
//! independently to avoid depending on prost.

mod decode;

pub use decode::decode_file_descriptor_set;

/// A collection of file descriptors.
/// Corresponds to google.protobuf.FileDescriptorSet.
#[derive(Debug, Clone, Default)]
pub struct FileDescriptorSet {
    /// The file descriptors.
    pub file: Vec<FileDescriptorProto>, // field 1
}

/// Describes a complete .proto file.
/// Corresponds to google.protobuf.FileDescriptorProto.
#[derive(Debug, Clone, Default)]
pub struct FileDescriptorProto {
    /// The file name, relative to root of source tree.
    pub name: Option<String>, // field 1
    /// The package name.
    pub package: Option<String>, // field 2
    /// Names of files imported by this file.
    pub dependency: Vec<String>, // field 3
    /// All top-level message definitions in this file.
    pub message_type: Vec<DescriptorProto>, // field 4
    /// All top-level enum definitions in this file.
    pub enum_type: Vec<EnumDescriptorProto>, // field 5
    /// The syntax of the proto file (e.g., "proto2", "proto3").
    pub syntax: Option<String>, // field 12
}

/// Describes a message type.
/// Corresponds to google.protobuf.DescriptorProto.
#[derive(Debug, Clone, Default)]
pub struct DescriptorProto {
    /// The message name.
    pub name: Option<String>, // field 1
    /// Fields of the message.
    pub field: Vec<FieldDescriptorProto>, // field 2
    /// Nested message types.
    pub nested_type: Vec<DescriptorProto>, // field 3
    /// Nested enum types.
    pub enum_type: Vec<EnumDescriptorProto>, // field 4
    /// Oneof declarations.
    pub oneof_decl: Vec<OneofDescriptorProto>, // field 8
    /// Message options.
    pub options: Option<MessageOptions>, // field 7
}

/// Describes a field within a message.
/// Corresponds to google.protobuf.FieldDescriptorProto.
#[derive(Debug, Clone, Default)]
pub struct FieldDescriptorProto {
    /// The field name.
    pub name: Option<String>, // field 1
    /// The field number (tag).
    pub number: Option<i32>, // field 3
    /// The field label (optional, required, repeated).
    pub label: Option<i32>, // field 4
    /// The field type.
    pub r#type: Option<i32>, // field 5
    /// For message and enum types, the fully-qualified type name.
    pub type_name: Option<String>, // field 6
    /// The default value as a string.
    pub default_value: Option<String>, // field 7
    /// Field options (includes protomon extensions).
    pub options: Option<FieldOptions>, // field 8
    /// If set, this field is part of a oneof.
    pub oneof_index: Option<i32>, // field 9
    /// The JSON name for this field.
    pub json_name: Option<String>, // field 10
    /// If true, this is a proto3 optional field.
    pub proto3_optional: Option<bool>, // field 17
}

impl FieldDescriptorProto {
    /// Get the field label.
    pub fn label(&self) -> Label {
        self.label.map(Label::from_i32).flatten().unwrap_or(Label::Optional)
    }

    /// Get the field type.
    pub fn field_type(&self) -> Option<Type> {
        self.r#type.map(Type::from_i32).flatten()
    }
}

/// Describes an enum type.
/// Corresponds to google.protobuf.EnumDescriptorProto.
#[derive(Debug, Clone, Default)]
pub struct EnumDescriptorProto {
    /// The enum name.
    pub name: Option<String>, // field 1
    /// The enum values.
    pub value: Vec<EnumValueDescriptorProto>, // field 2
}

/// Describes an enum value.
/// Corresponds to google.protobuf.EnumValueDescriptorProto.
#[derive(Debug, Clone, Default)]
pub struct EnumValueDescriptorProto {
    /// The value name.
    pub name: Option<String>, // field 1
    /// The value number.
    pub number: Option<i32>, // field 2
}

/// Describes a oneof.
/// Corresponds to google.protobuf.OneofDescriptorProto.
#[derive(Debug, Clone, Default)]
pub struct OneofDescriptorProto {
    /// The oneof name.
    pub name: Option<String>, // field 1
}

/// Options for a message type.
/// Corresponds to google.protobuf.MessageOptions.
#[derive(Debug, Clone, Default)]
pub struct MessageOptions {
    /// Set true if this message is a map entry type.
    pub map_entry: Option<bool>, // field 7
}

/// Options for a field.
/// Corresponds to google.protobuf.FieldOptions with protomon extensions.
#[derive(Debug, Clone, Default)]
pub struct FieldOptions {
    // Protomon extensions (field numbers 50001-50099)

    /// Use `Vec<T>` instead of `Repeated<T>` for repeated fields.
    /// Extension field 50001.
    pub vec: bool,

    /// Wrap field type in `Box<T>`.
    /// Extension field 50002.
    pub boxed: bool,

    /// Wrap message field in `LazyMessage<T>` for lazy/zero-copy decoding.
    /// Extension field 50003.
    pub lazy: bool,

    /// Use fixed-size array `[u8; N]` instead of `ProtoBytes` for bytes fields.
    /// Extension field 50004. Value of 0 means not set.
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
