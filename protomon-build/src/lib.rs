//! `protomon-build` compiles `.proto` files into Rust code for use with the
//! protomon library.
//!
//! # Example
//!
//! ```rust,no_run
//! // In build.rs
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     protomon_build::compile_protos(&["src/messages.proto"], &["src/"])?;
//!     Ok(())
//! }
//! ```
//!
//! # Customizing Code Generation
//!
//! Use protobuf extensions in your `.proto` files to customize generated code:
//!
//! ```protobuf
//! import "protomon/extensions.proto";
//!
//! message MyMessage {
//!   // Use `Vec<T>` instead of `Repeated<T>`
//!   repeated string tags = 1 [(protomon.vec) = true];
//!
//!   // Explicit `Box<T>` wrapping (optional - see auto-boxing below)
//!   optional MyMessage child = 2 [(protomon.boxed) = true];
//!
//!   // Use `LazyMessage<T>` for lazy/zero-copy decoding
//!   optional OtherMessage data = 3 [(protomon.lazy) = true];
//!
//!   // Use fixed-size array `[u8; N]` for bytes fields (max N=32)
//!   bytes hash = 4 [(protomon.fixed_array) = 32];  // [u8; 32]
//!
//!   // Use `Vec<u8>` instead of `ProtoBytes` for bytes fields
//!   bytes payload = 5 [(protomon.vec) = true];  // Vec<u8>
//! }
//! ```
//!
//! # Automatic Boxing of Recursive Types
//!
//! Recursive protobuf types (messages that reference themselves directly or
//! indirectly) are automatically detected and wrapped in `Box<T>` to ensure
//! the generated Rust types have a known size at compile time.
//!
//! ```protobuf
//! // These recursive fields are automatically boxed:
//! message Node {
//!   optional Node left = 1;   // Generates: Option<Box<Node>>
//!   optional Node right = 2;  // Generates: Option<Box<Node>>
//! }
//! ```
//!
//! You don't need to add `[(protomon.boxed) = true]` for recursive types -
//! the code generator handles this automatically.
//!
//! # Advanced Usage
//!
//! ```rust,no_run
//! fn main() -> Result<(), protomon_build::Error> {
//!     protomon_build::Config::new()
//!         .out_dir("src/proto")
//!         .extern_path(".google.protobuf.Timestamp", "prost_types::Timestamp")
//!         .compile_protos(&["proto/messages.proto"], &["proto/"])?;
//!     Ok(())
//! }
//! ```

mod codegen;
mod config;
mod context;
pub mod descriptor;
mod error;
mod protoc;

pub use config::Config;
pub use error::Error;

use std::path::Path;

/// Compile `.proto` files into Rust with default settings.
///
/// # Arguments
/// * `protos` - Paths to `.proto` files to compile
/// * `includes` - Include paths for resolving imports
///
/// # Example
///
/// ```rust,no_run
/// fn main() -> Result<(), protomon_build::Error> {
///     protomon_build::compile_protos(&["proto/messages.proto"], &["proto/"])?;
///     Ok(())
/// }
/// ```
pub fn compile_protos(
    protos: &[impl AsRef<Path>],
    includes: &[impl AsRef<Path>],
) -> Result<(), Error> {
    Config::new().compile_protos(protos, includes)
}
