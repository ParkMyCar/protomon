//! Protoc invocation utilities.

use crate::descriptor::{decode_file_descriptor_set, FileDescriptorSet};
use crate::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The embedded protomon extensions.proto file.
const EXTENSIONS_PROTO: &str = include_str!("../proto/protomon/extensions.proto");

/// Find the protoc executable.
pub fn find_protoc() -> Result<PathBuf, Error> {
    // Check PROTOC environment variable first
    if let Ok(path) = std::env::var("PROTOC") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }

    // Try to find protoc in PATH
    which::which("protoc").map_err(|_| Error::ProtocNotFound)
}

/// Invoke protoc to generate a FileDescriptorSet.
pub fn invoke_protoc(
    protoc: &Path,
    protos: &[impl AsRef<Path>],
    includes: &[impl AsRef<Path>],
    extra_args: &[String],
) -> Result<FileDescriptorSet, Error> {
    let tempdir = tempfile::tempdir()?;
    let descriptor_path = tempdir.path().join("descriptor.bin");

    // Write embedded protomon extensions.proto to temp directory
    let protomon_dir = tempdir.path().join("protomon");
    std::fs::create_dir_all(&protomon_dir)?;
    std::fs::write(protomon_dir.join("extensions.proto"), EXTENSIONS_PROTO)?;

    let mut cmd = Command::new(protoc);

    // Add protomon proto directory first (for extensions.proto)
    cmd.arg("-I").arg(tempdir.path());

    // Add user include paths
    for include in includes {
        cmd.arg("-I").arg(include.as_ref());
    }

    // Output descriptor set
    cmd.arg("--descriptor_set_out").arg(&descriptor_path);

    // Include imports so we have full type information
    cmd.arg("--include_imports");

    // Extra user-provided args
    for arg in extra_args {
        cmd.arg(arg);
    }

    // Proto files to compile
    for proto in protos {
        cmd.arg(proto.as_ref());
    }

    let output = cmd.output()?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Combine stdout and stderr for full error context
        let combined = if stdout.is_empty() {
            stderr.into_owned()
        } else if stderr.is_empty() {
            stdout.into_owned()
        } else {
            format!("{}\n{}", stdout, stderr)
        };
        return Err(Error::ProtocFailed(combined));
    }

    // Read and parse the descriptor set
    let descriptor_bytes = std::fs::read(&descriptor_path)?;
    decode_file_descriptor_set(&descriptor_bytes)
}

/// Parse a FileDescriptorSet from bytes.
pub fn parse_file_descriptor_set(bytes: &[u8]) -> Result<FileDescriptorSet, Error> {
    decode_file_descriptor_set(bytes)
}
