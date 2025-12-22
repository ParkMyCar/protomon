use core::fmt;

#[derive(Debug, Copy, Clone)]
pub enum DecodeErrorKind {
    InvalidWireType { value: u8 },
    InvalidKey { reason: &'static str },
    InvalidVarInt,
    UnexpectedEndOfBuffer,
    DeprecatedGroupEncoding,
    InvalidUtf8,
}

impl fmt::Display for DecodeErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
        }
    }
}
