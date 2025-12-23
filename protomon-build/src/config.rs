//! Configuration for protobuf code generation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuration for protobuf code generation.
///
/// Type customization (like using Vec vs Repeated) is done via protobuf extensions
/// in your .proto files. See `proto/protomon/extensions.proto` for available options.
#[derive(Debug, Clone)]
pub struct Config {
    /// Output directory for generated files.
    pub(crate) out_dir: Option<PathBuf>,

    /// Path to the protoc executable.
    pub(crate) protoc_path: Option<PathBuf>,

    /// Additional arguments for protoc.
    pub(crate) protoc_args: Vec<String>,

    /// Skip running protoc, use pre-existing FileDescriptorSet.
    pub(crate) skip_protoc: bool,

    /// Path to read/write FileDescriptorSet.
    pub(crate) file_descriptor_set_path: Option<PathBuf>,

    /// Extern paths for types defined elsewhere.
    /// Maps proto path -> Rust path (e.g., ".google.protobuf.Timestamp" -> "prost_types::Timestamp")
    pub(crate) extern_paths: HashMap<String, String>,

    /// Disable formatting with prettyplease.
    pub(crate) skip_format: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            out_dir: None,
            protoc_path: None,
            protoc_args: Vec::new(),
            skip_protoc: false,
            file_descriptor_set_path: None,
            extern_paths: HashMap::new(),
            skip_format: false,
        }
    }
}

impl Config {
    /// Create a new Config with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the output directory for generated Rust files.
    pub fn out_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.out_dir = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set path to the protoc executable.
    pub fn protoc_path(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.protoc_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Add an argument to pass to protoc.
    pub fn protoc_arg(&mut self, arg: impl Into<String>) -> &mut Self {
        self.protoc_args.push(arg.into());
        self
    }

    /// Skip running protoc; use an existing FileDescriptorSet instead.
    pub fn skip_protoc_run(&mut self) -> &mut Self {
        self.skip_protoc = true;
        self
    }

    /// Path to write/read the FileDescriptorSet.
    pub fn file_descriptor_set_path(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.file_descriptor_set_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Declare an externally provided protobuf type.
    ///
    /// When a field references a type matching `proto_path`, the generated code
    /// will use `rust_path` instead of generating the type.
    ///
    /// # Example
    /// ```ignore
    /// config.extern_path(".google.protobuf.Timestamp", "prost_types::Timestamp");
    /// ```
    pub fn extern_path(
        &mut self,
        proto_path: impl Into<String>,
        rust_path: impl Into<String>,
    ) -> &mut Self {
        self.extern_paths.insert(proto_path.into(), rust_path.into());
        self
    }

    /// Skip formatting with prettyplease.
    pub fn skip_format(&mut self) -> &mut Self {
        self.skip_format = true;
        self
    }

    /// Compile `.proto` files into Rust files.
    pub fn compile_protos(
        &self,
        protos: &[impl AsRef<Path>],
        includes: &[impl AsRef<Path>],
    ) -> Result<(), crate::Error> {
        crate::codegen::compile(self, protos, includes)
    }

    /// Compile from an existing FileDescriptorSet.
    pub fn compile_fds(
        &self,
        fds: crate::descriptor::FileDescriptorSet,
    ) -> Result<(), crate::Error> {
        crate::codegen::compile_fds(self, fds)
    }
}
