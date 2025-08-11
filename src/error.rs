use core::fmt;

#[derive(Debug, Copy, Clone)]
pub enum DecodeErrorKind {
    InvalidWireType { value: u8 },
    InvalidKey { reason: &'static str },
    InvalidVarInt,
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
        }
    }
}
