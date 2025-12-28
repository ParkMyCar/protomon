//! Code generation from protobuf descriptors.

mod enumeration;
mod field;
mod message;
mod module;
mod oneof;
mod recursion;
mod types;

pub use recursion::{find_recursive_fields, RecursiveField};

use std::collections::HashMap;
use std::path::Path;

use proc_macro2::TokenStream;

use crate::config::Config;
use crate::context::GenerationContext;
use crate::descriptor::{FileDescriptorProto, FileDescriptorSet};
use crate::protoc;
use crate::Error;

/// Main entry point for code generation.
pub fn compile(
    config: &Config,
    protos: &[impl AsRef<Path>],
    includes: &[impl AsRef<Path>],
) -> Result<(), Error> {
    let fds = if config.skip_protoc {
        // Load from file_descriptor_set_path
        let path = config
            .file_descriptor_set_path
            .as_ref()
            .ok_or(Error::MissingDescriptorPath)?;
        let bytes = std::fs::read(path)?;
        protoc::parse_file_descriptor_set(&bytes)?
    } else {
        let protoc_path = config
            .protoc_path
            .clone()
            .map(Ok)
            .unwrap_or_else(protoc::find_protoc)?;
        protoc::invoke_protoc(&protoc_path, protos, includes, &config.protoc_args)?
    };

    // Optionally save FileDescriptorSet
    if let Some(_path) = &config.file_descriptor_set_path {
        if !config.skip_protoc {
            // Note: We don't have encode functionality, so we skip this for now
            // This could be added later if needed
        }
    }

    compile_fds(config, fds)
}

/// Compile from a FileDescriptorSet.
pub fn compile_fds(config: &Config, fds: FileDescriptorSet) -> Result<(), Error> {
    let out_dir = config
        .out_dir
        .clone()
        .or_else(|| std::env::var_os("OUT_DIR").map(Into::into))
        .ok_or(Error::MissingOutDir)?;

    // Build context with type registry
    let ctx = GenerationContext::new(config, &fds);

    // Generate code for each file
    let mut modules: HashMap<String, TokenStream> = HashMap::new();

    for file in &fds.file {
        let code = generate_file(&ctx, file)?;
        let module_path = file_to_module_path(file);
        // Extend existing module content if multiple files share the same package
        modules.entry(module_path).or_default().extend(code);
    }

    // Write output files
    module::write_modules(&out_dir, &modules, config.skip_format)?;

    Ok(())
}

/// Generate code for a single .proto file.
fn generate_file(
    ctx: &GenerationContext,
    file: &FileDescriptorProto,
) -> Result<TokenStream, Error> {
    let package = file.package.as_deref().unwrap_or("");
    let syntax = file.syntax.as_deref().unwrap_or("proto2");
    let is_proto3 = syntax == "proto3";

    let mut tokens = TokenStream::new();

    // Generate enums
    for enum_type in &file.enum_type {
        let enum_tokens = enumeration::generate_enum(&format!(".{}", package), enum_type)?;
        tokens.extend(enum_tokens);
    }

    // Generate messages (including nested)
    for msg in &file.message_type {
        let msg_tokens = message::generate_message(ctx, &format!(".{}", package), msg, is_proto3)?;
        tokens.extend(msg_tokens);
    }

    Ok(tokens)
}

/// Convert file descriptor to module path.
fn file_to_module_path(file: &FileDescriptorProto) -> String {
    // Use package name, or filename without extension
    if let Some(package) = file.package.as_ref().filter(|p| !p.is_empty()) {
        package.replace('.', "_")
    } else {
        let name = file.name.as_deref().unwrap_or("unknown");
        Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }
}
