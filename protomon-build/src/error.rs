//! Error types for protomon-build.

use std::io;

/// Errors that can occur during protobuf code generation.
#[derive(Debug)]
pub enum Error {
    /// IO error.
    Io(io::Error),
    /// protoc not found.
    ProtocNotFound,
    /// protoc invocation failed.
    ProtocFailed(String),
    /// Failed to decode FileDescriptorSet.
    DecodeError(String),
    /// Missing OUT_DIR environment variable.
    MissingOutDir,
    /// Missing file_descriptor_set_path when skip_protoc is set.
    MissingDescriptorPath,
    /// Missing name field in descriptor.
    MissingName,
    /// Missing field number.
    MissingFieldNumber,
    /// Invalid field type.
    InvalidFieldType(i32),
    /// Invalid label.
    InvalidLabel(i32),
    /// Syn parse error.
    SynParse(String),
    /// Invalid varint encoding.
    InvalidVarint,
    /// Unexpected end of buffer.
    UnexpectedEof,
    /// Invalid wire type.
    InvalidWireType(u8),
    /// Invalid UTF-8 in string field.
    InvalidUtf8,
    /// Invalid protomon extension option usage.
    InvalidOption(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::ProtocNotFound => {
                write!(f, "protoc not found. Set PROTOC env var or install protoc.")
            }
            Self::ProtocFailed(msg) => {
                // Truncate very long error messages to keep output readable
                const MAX_LEN: usize = 1000;
                if msg.len() > MAX_LEN {
                    write!(f, "protoc failed: {}... (truncated)", &msg[..MAX_LEN])
                } else {
                    write!(f, "protoc failed: {}", msg)
                }
            }
            Self::DecodeError(msg) => write!(f, "Failed to decode FileDescriptorSet: {}", msg),
            Self::MissingOutDir => {
                write!(f, "OUT_DIR not set. Run from build.rs or set out_dir().")
            }
            Self::MissingDescriptorPath => {
                write!(
                    f,
                    "file_descriptor_set_path required when skip_protoc is set"
                )
            }
            Self::MissingName => write!(f, "Missing name in descriptor"),
            Self::MissingFieldNumber => write!(f, "Missing field number in descriptor"),
            Self::InvalidFieldType(t) => write!(f, "Invalid field type: {} (expected 1-18)", t),
            Self::InvalidLabel(l) => write!(f, "Invalid field label: {} (expected 1-3)", l),
            Self::SynParse(msg) => write!(f, "Failed to parse generated code: {}", msg),
            Self::InvalidVarint => write!(f, "Invalid varint encoding"),
            Self::UnexpectedEof => write!(f, "Unexpected end of buffer"),
            Self::InvalidWireType(w) => write!(f, "Invalid wire type: {}", w),
            Self::InvalidUtf8 => write!(f, "Invalid UTF-8 in string field"),
            Self::InvalidOption(msg) => write!(f, "Invalid protomon option: {}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(_: std::string::FromUtf8Error) -> Self {
        Self::InvalidUtf8
    }
}
