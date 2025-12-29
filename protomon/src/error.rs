use alloc::boxed::Box;
use core::fmt;

/// Compact decode error optimized for register returns.
///
/// # Performance
///
/// This error type is exactly 8 bytes (a `Box` pointer), enabling
/// `Result<T, DecodeError>` to be returned in registers for common return types:
///
/// - `Result<(WireType, u32), DecodeError>` = 16 bytes → RAX:RDX registers
/// - `Result<usize, DecodeError>` = 16 bytes → RAX:RDX registers
/// - `Result<(), DecodeError>` = 8 bytes → RAX register
///
/// The x86-64 System V ABI returns values up to 16 bytes in RAX:RDX registers.
/// Values larger than 16 bytes are returned via a hidden stack pointer (sret),
/// which adds memory access overhead on every call.
///
/// The heap allocation only occurs on the error path, which is cold. Since
/// errors are exceptional, this trade-off is worthwhile for the hot path gains.
#[derive(Debug, Clone)]
pub struct DecodeError(Box<DecodeErrorInner>);

/// Inner error type containing full error details.
#[derive(Debug, Clone)]
pub enum DecodeErrorInner {
    InvalidWireType { value: u8 },
    InvalidKey { reason: &'static str },
    InvalidVarInt,
    UnexpectedEndOfBuffer,
    DeprecatedGroupEncoding,
    InvalidUtf8,
    LengthOverflow { value: u64 },
    LengthMismatch { expected: usize, actual: usize },
    ProgrammingError { reason: &'static str },
    MissingRequiredOneof { field: &'static str },
    InvalidPackedLength { expected_multiple: u8, actual: u32 },
    IntegerOverflow { target_type: &'static str },
}

impl DecodeError {
    /// Get the inner error kind.
    #[inline]
    pub fn kind(&self) -> &DecodeErrorInner {
        &self.0
    }

    /// Convert into the inner error kind.
    #[inline]
    pub fn into_inner(self) -> DecodeErrorInner {
        *self.0
    }

    // Cold constructors - heap allocation happens here, on the error path.
    // These are marked #[cold] #[inline(never)] to keep error construction
    // out of the hot path instruction cache.

    #[cold]
    #[inline(never)]
    pub fn invalid_wire_type(value: u8) -> Self {
        Self(Box::new(DecodeErrorInner::InvalidWireType { value }))
    }

    #[cold]
    #[inline(never)]
    pub fn invalid_key(reason: &'static str) -> Self {
        Self(Box::new(DecodeErrorInner::InvalidKey { reason }))
    }

    #[cold]
    #[inline(never)]
    pub fn invalid_varint() -> Self {
        Self(Box::new(DecodeErrorInner::InvalidVarInt))
    }

    #[cold]
    #[inline(never)]
    pub fn unexpected_end_of_buffer() -> Self {
        Self(Box::new(DecodeErrorInner::UnexpectedEndOfBuffer))
    }

    #[cold]
    #[inline(never)]
    pub fn deprecated_group_encoding() -> Self {
        Self(Box::new(DecodeErrorInner::DeprecatedGroupEncoding))
    }

    #[cold]
    #[inline(never)]
    pub fn invalid_utf8() -> Self {
        Self(Box::new(DecodeErrorInner::InvalidUtf8))
    }

    #[cold]
    #[inline(never)]
    pub fn length_overflow(value: u64) -> Self {
        Self(Box::new(DecodeErrorInner::LengthOverflow { value }))
    }

    #[cold]
    #[inline(never)]
    pub fn length_mismatch(expected: usize, actual: usize) -> Self {
        Self(Box::new(DecodeErrorInner::LengthMismatch { expected, actual }))
    }

    #[cold]
    #[inline(never)]
    pub fn programming_error(reason: &'static str) -> Self {
        Self(Box::new(DecodeErrorInner::ProgrammingError { reason }))
    }

    #[cold]
    #[inline(never)]
    pub fn missing_required_oneof(field: &'static str) -> Self {
        Self(Box::new(DecodeErrorInner::MissingRequiredOneof { field }))
    }

    #[cold]
    #[inline(never)]
    pub fn invalid_packed_length(expected_multiple: u8, actual: u32) -> Self {
        Self(Box::new(DecodeErrorInner::InvalidPackedLength {
            expected_multiple,
            actual,
        }))
    }

    #[cold]
    #[inline(never)]
    pub fn integer_overflow(target_type: &'static str) -> Self {
        Self(Box::new(DecodeErrorInner::IntegerOverflow { target_type }))
    }
}

impl fmt::Display for DecodeError {
    #[inline(never)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            DecodeErrorInner::InvalidWireType { value } => {
                write!(f, "invalid wire type value: {value}")
            }
            DecodeErrorInner::InvalidKey { reason } => {
                write!(f, "invalid key: '{reason}'")
            }
            DecodeErrorInner::InvalidVarInt => {
                write!(f, "invalid leb128 varint")
            }
            DecodeErrorInner::UnexpectedEndOfBuffer => {
                write!(f, "unexpected end of buffer")
            }
            DecodeErrorInner::DeprecatedGroupEncoding => {
                write!(f, "deprecated group encoding not supported")
            }
            DecodeErrorInner::InvalidUtf8 => {
                write!(f, "invalid UTF-8 in string field")
            }
            DecodeErrorInner::LengthOverflow { value } => {
                write!(
                    f,
                    "length prefix {value} exceeds platform addressable memory"
                )
            }
            DecodeErrorInner::LengthMismatch { expected, actual } => {
                write!(f, "length mismatch: expected {expected}, got {actual}")
            }
            DecodeErrorInner::ProgrammingError { reason } => {
                write!(f, "programming error: '{reason}'")
            }
            DecodeErrorInner::MissingRequiredOneof { field } => {
                write!(f, "missing required oneof field: '{field}'")
            }
            DecodeErrorInner::InvalidPackedLength {
                expected_multiple,
                actual,
            } => {
                write!(
                    f,
                    "invalid packed field length: {actual} is not a multiple of {expected_multiple}"
                )
            }
            DecodeErrorInner::IntegerOverflow { target_type } => {
                write!(f, "integer overflow: value does not fit in {target_type}")
            }
        }
    }
}

// Keep the old type name as an alias for backwards compatibility
pub type DecodeErrorKind = DecodeError;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use core::mem::size_of;

    #[test]
    fn test_error_size() {
        // Box<T> is exactly 8 bytes (pointer)
        assert_eq!(size_of::<DecodeError>(), 8);

        // Result types should be small enough for register returns (≤16 bytes)
        assert_eq!(size_of::<Result<(), DecodeError>>(), 8);
        assert_eq!(size_of::<Result<usize, DecodeError>>(), 16);
        assert_eq!(size_of::<Result<(u8, u32), DecodeError>>(), 16);

        // Option<DecodeError> should use niche optimization
        assert_eq!(size_of::<Option<DecodeError>>(), 8);
    }

    #[test]
    fn test_error_display() {
        let err = DecodeError::invalid_wire_type(7);
        assert_eq!(format!("{err}"), "invalid wire type value: 7");

        let err = DecodeError::length_mismatch(100, 200);
        assert_eq!(format!("{err}"), "length mismatch: expected 100, got 200");

        let err = DecodeError::invalid_key("empty buffer");
        assert_eq!(format!("{err}"), "invalid key: 'empty buffer'");
    }

    #[test]
    fn test_error_kind() {
        let err = DecodeError::invalid_varint();
        assert!(matches!(err.kind(), DecodeErrorInner::InvalidVarInt));

        let err = DecodeError::invalid_packed_length(4, 15);
        assert!(matches!(
            err.kind(),
            DecodeErrorInner::InvalidPackedLength {
                expected_multiple: 4,
                actual: 15
            }
        ));
    }
}
