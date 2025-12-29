//! Compact error type for protobuf decoding.
//!
//! All bit manipulation in this module is intentional for packing error info
//! into a single 64-bit value for register returns.

// Allow as_conversions for intentional bit-packing operations.
#![allow(clippy::as_conversions)]

use core::fmt;
use core::num::NonZeroU64;

/// Decode error type packed into 8 bytes.
///
/// # Layout
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │ 63       56 │ 55                                              0 │
/// │   kind (8)  │              context (56 bits)                    │
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// - Bits 56-63: Error kind discriminant (1-255, 0 reserved for niche)
/// - Bits 0-55: Context data (interpretation depends on kind)
///
/// # Performance
///
/// This error type is exactly 8 bytes ([`NonZeroU64`]) which enables
/// `Result<T, DecodeError>` to be returned entirely in registers.
///
/// Alternatively we could do something like `Box<DecodeErrorInner>` which
/// would be 1 word but this requires an allocation. Errors are a cold path so
/// allocating is not bad, but this prevents zero-allocation decoding which is
/// desirable for platforms without an allocator.
///
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct DecodeError(NonZeroU64);

// Ensure that we're sized such that we can return a result in registers.
crate::util::assert_eq_size!(Result<u64, DecodeError>, [u8; 16]);
// Ensure that a niche value exists for DecodeError.
crate::util::assert_eq_size!(Option<DecodeError>, DecodeError);

/// Error kind discriminants (stored in upper 8 bits).
///
/// Values start at 1 because 0 is reserved for niche optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorKind {
    InvalidWireType = 1,
    InvalidKey = 2,
    InvalidVarInt = 3,
    UnexpectedEndOfBuffer = 4,
    DeprecatedGroupEncoding = 5,
    InvalidUtf8 = 6,
    LengthOverflow = 7,
    LengthMismatch = 8,
    ProgrammingError = 9,
    MissingRequiredOneof = 10,
    InvalidPackedLength = 11,
    IntegerOverflow = 12,
}

/// Reason codes for [`ErrorKind::InvalidKey`] errors (stored in context bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InvalidKeyReason {
    EmptyBuffer = 1,
    TagOutOfRange = 2,
}

/// Reason codes for [`ErrorKind::ProgrammingError`] (stored in context bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProgrammingErrorReason {
    DecodeIntoOwnedNotSupported = 1,
    InitRepeatedNotCalled = 2,
}

/// Target types for [`ErrorKind::IntegerOverflow`] errors (stored in context bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OverflowTargetType {
    I32 = 1,
    U32 = 2,
    I64 = 3,
    U64 = 4,
    Usize = 5,
}

// Bit manipulation constants
const KIND_SHIFT: u32 = 56;
const CONTEXT_MASK: u64 = (1 << KIND_SHIFT) - 1; // Lower 56 bits

impl DecodeError {
    /// Create a [`DecodeError`] from kind and context.
    #[inline(always)]
    const fn new(kind: ErrorKind, context: u64) -> Self {
        let value = ((kind as u64) << KIND_SHIFT) | (context & CONTEXT_MASK);
        // SAFETY: kind is always >= 1, so the upper byte is never 0
        Self(unsafe { NonZeroU64::new_unchecked(value) })
    }

    /// Create a [`DecodeError`] with no context.
    #[inline(always)]
    const fn new_simple(kind: ErrorKind) -> Self {
        Self::new(kind, 0)
    }

    /// Extracts the [`ErrorKind`].
    #[inline(always)]
    pub const fn kind(&self) -> ErrorKind {
        let kind_byte = (self.0.get() >> KIND_SHIFT) as u8;
        // SAFETY: We only construct with valid ErrorKind values
        unsafe { core::mem::transmute::<u8, ErrorKind>(kind_byte) }
    }

    /// Extracts the the raw context bits (lower 56 bits).
    #[inline(always)]
    const fn context(&self) -> u64 {
        self.0.get() & CONTEXT_MASK
    }

    /// Construct an "invalid wire type" error with the provided value as context.
    #[cold]
    #[inline(never)]
    pub const fn invalid_wire_type(value: u8) -> Self {
        Self::new(ErrorKind::InvalidWireType, value as u64)
    }

    /// Construct an "invalid key" error with [`InvalidKeyReason`] as context.
    #[cold]
    #[inline(never)]
    pub const fn invalid_key(reason: InvalidKeyReason) -> Self {
        Self::new(ErrorKind::InvalidKey, reason as u64)
    }

    /// Encountered an invalid varint.
    #[cold]
    #[inline(never)]
    pub const fn invalid_varint() -> Self {
        Self::new_simple(ErrorKind::InvalidVarInt)
    }

    /// Unexpectedly reached the end of a buffer.
    #[cold]
    #[inline(never)]
    pub const fn unexpected_end_of_buffer() -> Self {
        Self::new_simple(ErrorKind::UnexpectedEndOfBuffer)
    }

    /// A deprecated group encoding encountered.
    #[cold]
    #[inline(never)]
    pub const fn deprecated_group_encoding() -> Self {
        Self::new_simple(ErrorKind::DeprecatedGroupEncoding)
    }

    /// Invalid UTF-8 in string field.
    #[cold]
    #[inline(never)]
    pub const fn invalid_utf8() -> Self {
        Self::new_simple(ErrorKind::InvalidUtf8)
    }

    /// Length prefix exceeds platform addressable memory.
    ///
    /// Note: This should only occur on machines with a 32-bit architecture.
    #[cold]
    #[inline(never)]
    pub const fn length_overflow(value: u64) -> Self {
        Self::new(ErrorKind::LengthOverflow, value)
    }

    /// Length mismatch for fixed-size array.
    ///
    /// Note: The `expected` and `actual` lengths are truncated to 16 bits to fit in the
    /// error's context.
    #[cold]
    #[inline(never)]
    #[allow(clippy::as_conversions)]
    pub const fn length_mismatch(expected: usize, actual: usize) -> Self {
        let expected_u16 = expected as u16;
        let actual_u16 = actual as u16;
        let context = ((expected_u16 as u64) << 16) | (actual_u16 as u64);
        Self::new(ErrorKind::LengthMismatch, context)
    }

    /// Programming error.
    #[cold]
    #[inline(never)]
    pub const fn programming_error(reason: ProgrammingErrorReason) -> Self {
        Self::new(ErrorKind::ProgrammingError, reason as u64)
    }

    /// Missing required oneof field.
    #[cold]
    #[inline(never)]
    pub const fn missing_required_oneof(tag: u32) -> Self {
        Self::new(ErrorKind::MissingRequiredOneof, tag as u64)
    }

    /// Invalid packed field length.
    #[cold]
    #[inline(never)]
    pub const fn invalid_packed_length(expected_multiple: u8, actual: u32) -> Self {
        let context = ((expected_multiple as u64) << 32) | (actual as u64);
        Self::new(ErrorKind::InvalidPackedLength, context)
    }

    /// Integer overflow during conversion.
    #[cold]
    #[inline(never)]
    pub const fn integer_overflow(target: OverflowTargetType) -> Self {
        Self::new(ErrorKind::IntegerOverflow, target as u64)
    }

    /// Get the context for an [`ErrorKind::InvalidWireType`] error.
    pub(crate) const fn wire_type_value(&self) -> Option<u8> {
        if matches!(self.kind(), ErrorKind::InvalidWireType) {
            Some(self.context() as u8)
        } else {
            None
        }
    }

    /// Get the context for an [`ErrorKind::InvalidKey`] error.
    pub(crate) const fn invalid_key_reason(&self) -> Option<InvalidKeyReason> {
        if matches!(self.kind(), ErrorKind::InvalidKey) {
            let reason = self.context() as u8;
            // SAFETY: We only store valid InvalidKeyReason values
            Some(unsafe { core::mem::transmute::<u8, InvalidKeyReason>(reason) })
        } else {
            None
        }
    }

    /// Get the context for an [`ErrorKind::LengthOverflow`] error.
    pub(crate) const fn overflow_value(&self) -> Option<u64> {
        if matches!(self.kind(), ErrorKind::LengthOverflow) {
            Some(self.context())
        } else {
            None
        }
    }

    /// Get the context for an [`ErrorKind::LengthMismatch`] error.
    pub(crate) const fn length_mismatch_values(&self) -> Option<(u16, u16)> {
        if matches!(self.kind(), ErrorKind::LengthMismatch) {
            let ctx = self.context();
            let expected = (ctx >> 16) as u16;
            let actual = ctx as u16;
            Some((expected, actual))
        } else {
            None
        }
    }

    /// Get the context for an [`ErrorKind::ProgrammingError`].
    pub(crate) const fn programming_error_reason(&self) -> Option<ProgrammingErrorReason> {
        if matches!(self.kind(), ErrorKind::ProgrammingError) {
            let reason = self.context() as u8;
            Some(unsafe { core::mem::transmute::<u8, ProgrammingErrorReason>(reason) })
        } else {
            None
        }
    }

    /// Get the context for an [`ErrorKind::MissingRequiredOneof`] error.
    pub(crate) const fn missing_oneof_tag(&self) -> Option<u32> {
        if matches!(self.kind(), ErrorKind::MissingRequiredOneof) {
            Some(self.context() as u32)
        } else {
            None
        }
    }

    /// Get the context for an [`ErrorKind::InvalidPackedLength`] error.
    pub(crate) const fn packed_length_values(&self) -> Option<(u8, u32)> {
        if matches!(self.kind(), ErrorKind::InvalidPackedLength) {
            let ctx = self.context();
            let expected = (ctx >> 32) as u8;
            let actual = ctx as u32;
            Some((expected, actual))
        } else {
            None
        }
    }

    /// Get IntegerOverflow context: the target type.
    pub(crate) const fn overflow_target(&self) -> Option<OverflowTargetType> {
        if matches!(self.kind(), ErrorKind::IntegerOverflow) {
            let target = self.context() as u8;
            Some(unsafe { core::mem::transmute::<u8, OverflowTargetType>(target) })
        } else {
            None
        }
    }
}

impl fmt::Debug for DecodeError {
    #[cold]
    #[inline(never)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("DecodeError");
        d.field("kind", &self.kind());

        match self.kind() {
            ErrorKind::InvalidWireType => {
                d.field("value", &self.wire_type_value().unwrap());
            }
            ErrorKind::InvalidKey => {
                d.field("reason", &self.invalid_key_reason().unwrap());
            }
            ErrorKind::LengthOverflow => {
                d.field("value", &self.overflow_value().unwrap());
            }
            ErrorKind::LengthMismatch => {
                let (expected, actual) = self.length_mismatch_values().unwrap();
                d.field("expected", &expected);
                d.field("actual", &actual);
            }
            ErrorKind::ProgrammingError => {
                d.field("reason", &self.programming_error_reason().unwrap());
            }
            ErrorKind::MissingRequiredOneof => {
                d.field("tag", &self.missing_oneof_tag().unwrap());
            }
            ErrorKind::InvalidPackedLength => {
                let (mult, actual) = self.packed_length_values().unwrap();
                d.field("expected_multiple", &mult);
                d.field("actual", &actual);
            }
            ErrorKind::IntegerOverflow => {
                d.field("target", &self.overflow_target().unwrap());
            }
            _ => {}
        }

        d.finish()
    }
}

impl fmt::Display for DecodeError {
    #[cold]
    #[inline(never)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            ErrorKind::InvalidWireType => {
                write!(f, "invalid wire type value: {}", self.context() as u8)
            }
            ErrorKind::InvalidKey => {
                let reason = match self.invalid_key_reason() {
                    Some(InvalidKeyReason::EmptyBuffer) => "empty buffer",
                    Some(InvalidKeyReason::TagOutOfRange) => "tag out of range",
                    None => "unknown",
                };
                write!(f, "invalid key: {reason}")
            }
            ErrorKind::InvalidVarInt => {
                write!(f, "invalid leb128 varint")
            }
            ErrorKind::UnexpectedEndOfBuffer => {
                write!(f, "unexpected end of buffer")
            }
            ErrorKind::DeprecatedGroupEncoding => {
                write!(f, "deprecated group encoding not supported")
            }
            ErrorKind::InvalidUtf8 => {
                write!(f, "invalid UTF-8 in string field")
            }
            ErrorKind::LengthOverflow => {
                write!(
                    f,
                    "length prefix {} exceeds platform addressable memory",
                    self.context()
                )
            }
            ErrorKind::LengthMismatch => {
                let (expected, actual) = self.length_mismatch_values().unwrap();
                write!(f, "length mismatch: expected {expected}, got {actual}")
            }
            ErrorKind::ProgrammingError => {
                let reason = match self.programming_error_reason() {
                    Some(ProgrammingErrorReason::DecodeIntoOwnedNotSupported) => {
                        "decode_into is not supported on Owned variant"
                    }
                    Some(ProgrammingErrorReason::InitRepeatedNotCalled) => {
                        "Repeated::init_repeated must be called before decode_into"
                    }
                    None => "unknown",
                };
                write!(f, "programming error: {reason}")
            }
            ErrorKind::MissingRequiredOneof => {
                write!(
                    f,
                    "missing required oneof field (tag {})",
                    self.missing_oneof_tag().unwrap()
                )
            }
            ErrorKind::InvalidPackedLength => {
                let (mult, actual) = self.packed_length_values().unwrap();
                write!(
                    f,
                    "invalid packed field length: {actual} is not a multiple of {mult}"
                )
            }
            ErrorKind::IntegerOverflow => {
                let target = match self.overflow_target() {
                    Some(OverflowTargetType::I32) => "i32",
                    Some(OverflowTargetType::U32) => "u32",
                    Some(OverflowTargetType::I64) => "i64",
                    Some(OverflowTargetType::U64) => "u64",
                    Some(OverflowTargetType::Usize) => "usize",
                    None => "unknown",
                };
                write!(f, "integer overflow: value does not fit in {target}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn test_error_display() {
        let err = DecodeError::invalid_wire_type(7);
        assert_eq!(format!("{err}"), "invalid wire type value: 7");

        let err = DecodeError::length_mismatch(100, 200);
        assert_eq!(format!("{err}"), "length mismatch: expected 100, got 200");

        let err = DecodeError::invalid_key(InvalidKeyReason::EmptyBuffer);
        assert_eq!(format!("{err}"), "invalid key: empty buffer");

        let err = DecodeError::invalid_key(InvalidKeyReason::TagOutOfRange);
        assert_eq!(format!("{err}"), "invalid key: tag out of range");
    }

    #[test]
    fn test_error_kind() {
        let err = DecodeError::invalid_varint();
        assert_eq!(err.kind(), ErrorKind::InvalidVarInt);

        let err = DecodeError::invalid_packed_length(4, 15);
        assert_eq!(err.kind(), ErrorKind::InvalidPackedLength);
        assert_eq!(err.packed_length_values(), Some((4, 15)));
    }

    #[test]
    fn test_context_extraction() {
        // InvalidWireType
        let err = DecodeError::invalid_wire_type(99);
        assert_eq!(err.wire_type_value(), Some(99));

        // InvalidKey
        let err = DecodeError::invalid_key(InvalidKeyReason::TagOutOfRange);
        assert_eq!(
            err.invalid_key_reason(),
            Some(InvalidKeyReason::TagOutOfRange)
        );

        // LengthOverflow - note: only lower 56 bits preserved
        let err = DecodeError::length_overflow(12345);
        assert_eq!(err.overflow_value(), Some(12345));

        // LengthMismatch - truncated to u16
        let err = DecodeError::length_mismatch(100, 200);
        assert_eq!(err.length_mismatch_values(), Some((100, 200)));

        // ProgrammingError
        let err = DecodeError::programming_error(ProgrammingErrorReason::InitRepeatedNotCalled);
        assert_eq!(
            err.programming_error_reason(),
            Some(ProgrammingErrorReason::InitRepeatedNotCalled)
        );

        // MissingRequiredOneof
        let err = DecodeError::missing_required_oneof(42);
        assert_eq!(err.missing_oneof_tag(), Some(42));

        // InvalidPackedLength
        let err = DecodeError::invalid_packed_length(8, 1000);
        assert_eq!(err.packed_length_values(), Some((8, 1000)));

        // IntegerOverflow
        let err = DecodeError::integer_overflow(OverflowTargetType::I32);
        assert_eq!(err.overflow_target(), Some(OverflowTargetType::I32));
    }
}
