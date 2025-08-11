use core::fmt;

#[derive(Debug)]
pub struct DecodeError;

#[derive(Debug)]
pub enum DecodeErrorKind {
    InvalidWireType { value: u64 },
}

impl fmt::Display for DecodeErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeErrorKind::InvalidWireType { value } => {
                write!(f, "invalid 'wire type' value: {value}")
            }
        }
    }
}
