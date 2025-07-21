#[derive(Debug)]
pub struct DecodeError;

#[derive(Debug, thiserror::Error)]
pub enum DecodeErrorKind {
    #[error("invalid 'wire type' value: {value}'")]
    InvalidWireType { value: u64 },
}
