//! Efficient default value checking for protobuf types.
//!
//! In proto3, fields with default values are not encoded. This trait provides
//! efficient ways to check if a value is the default without allocating.

/// Trait for efficiently checking if a value is the protobuf default.
///
/// This is more efficient than `self == Default::default()` because it avoids
/// creating a temporary default value for comparison.
pub trait IsProtoDefault {
    /// Returns true if this value is the protobuf default value.
    fn is_proto_default(&self) -> bool;
}

// Primitive integer types - compare against 0
impl IsProtoDefault for u32 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        *self == 0
    }
}

impl IsProtoDefault for u64 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        *self == 0
    }
}

impl IsProtoDefault for i32 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        *self == 0
    }
}

impl IsProtoDefault for i64 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        *self == 0
    }
}

// Bool - default is false
impl IsProtoDefault for bool {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        !*self
    }
}

// Floating point - default is 0.0
impl IsProtoDefault for f32 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        *self == 0.0
    }
}

impl IsProtoDefault for f64 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        *self == 0.0
    }
}

// Wrapper types - check inner value
impl IsProtoDefault for super::Sint32 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.0 == 0
    }
}

impl IsProtoDefault for super::Sint64 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.0 == 0
    }
}

impl IsProtoDefault for super::Fixed32 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.0 == 0
    }
}

impl IsProtoDefault for super::Fixed64 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.0 == 0
    }
}

impl IsProtoDefault for super::Sfixed32 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.0 == 0
    }
}

impl IsProtoDefault for super::Sfixed64 {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.0 == 0
    }
}

// String types - check if empty
impl IsProtoDefault for super::ProtoString {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.is_empty()
    }
}

impl IsProtoDefault for super::ProtoBytes {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.is_empty()
    }
}

impl IsProtoDefault for String {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.is_empty()
    }
}

impl IsProtoDefault for Vec<u8> {
    #[inline(always)]
    fn is_proto_default(&self) -> bool {
        self.is_empty()
    }
}
