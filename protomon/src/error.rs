use core::fmt;

#[derive(Debug, Copy, Clone)]
pub enum DecodeErrorKind {
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

impl fmt::Display for DecodeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeErrorKind::InvalidWireType { value } => {
                write!(f, "invalid 'wire type' value: {value}")
            }
            DecodeErrorKind::InvalidKey { reason } => {
                write!(f, "invalid key: '{reason}'")
            }
            DecodeErrorKind::InvalidVarInt => {
                write!(f, "invalid leb128 varint")
            }
            DecodeErrorKind::UnexpectedEndOfBuffer => {
                write!(f, "unexpected end of buffer")
            }
            DecodeErrorKind::DeprecatedGroupEncoding => {
                write!(f, "deprecated group encoding not supported")
            }
            DecodeErrorKind::InvalidUtf8 => {
                write!(f, "invalid UTF-8 in string field")
            }
            DecodeErrorKind::LengthOverflow { value } => {
                write!(
                    f,
                    "length prefix {value} exceeds platform addressable memory"
                )
            }
            DecodeErrorKind::LengthMismatch { expected, actual } => {
                write!(f, "length mismatch: expected {expected}, got {actual}")
            }
            DecodeErrorKind::ProgrammingError { reason } => {
                write!(f, "programming error: '{reason}'")
            }
            DecodeErrorKind::MissingRequiredOneof { field } => {
                write!(f, "missing required oneof field: '{field}'")
            }
            DecodeErrorKind::InvalidPackedLength {
                expected_multiple,
                actual,
            } => {
                write!(
                    f,
                    "invalid packed field length: {actual} is not a multiple of {expected_multiple}"
                )
            }
            DecodeErrorKind::IntegerOverflow { target_type } => {
                write!(f, "integer overflow: value does not fit in {target_type}")
            }
        }
    }
}
