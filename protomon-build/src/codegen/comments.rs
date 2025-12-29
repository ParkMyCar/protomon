//! Comment extraction from protobuf SourceCodeInfo.

use std::collections::HashMap;

use crate::descriptor::{FileDescriptorProto, Location};

/// Field numbers from google/protobuf/descriptor.proto.
/// These are used to construct paths into the descriptor tree.
mod field_numbers {
    /// FileDescriptorProto.message_type
    pub const MESSAGE_TYPE: i32 = 4;
    /// FileDescriptorProto.enum_type
    pub const ENUM_TYPE: i32 = 5;
    /// DescriptorProto.field
    pub const FIELD: i32 = 2;
    /// DescriptorProto.nested_type
    pub const NESTED_TYPE: i32 = 3;
    /// DescriptorProto.enum_type (nested)
    pub const NESTED_ENUM_TYPE: i32 = 4;
    /// DescriptorProto.oneof_decl
    pub const ONEOF_DECL: i32 = 8;
    /// EnumDescriptorProto.value
    pub const ENUM_VALUE: i32 = 2;
}

/// A path to a location in the protobuf descriptor tree.
///
/// Protobuf's SourceCodeInfo uses paths (arrays of field numbers) to identify
/// locations in the descriptor. This type provides a type-safe builder API
/// for constructing these paths.
///
/// # Example
///
/// ```ignore
/// // Path to the second field of the first message
/// let path = DescriptorPath::message(0).field(1);
///
/// // Path to a nested enum value
/// let path = DescriptorPath::message(0).nested_enum(0).enum_value(2);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DescriptorPath(Vec<i32>);

impl DescriptorPath {
    /// Create a path to a top-level message.
    pub fn message(index: usize) -> Self {
        Self(vec![field_numbers::MESSAGE_TYPE, index as i32])
    }

    /// Create a path to a top-level enum.
    pub fn top_level_enum(index: usize) -> Self {
        Self(vec![field_numbers::ENUM_TYPE, index as i32])
    }

    /// Extend this path to a field within a message.
    pub fn field(&self, index: usize) -> Self {
        let mut path = self.0.clone();
        path.push(field_numbers::FIELD);
        path.push(index as i32);
        Self(path)
    }

    /// Extend this path to a nested message.
    pub fn nested_message(&self, index: usize) -> Self {
        let mut path = self.0.clone();
        path.push(field_numbers::NESTED_TYPE);
        path.push(index as i32);
        Self(path)
    }

    /// Extend this path to a nested enum.
    pub fn nested_enum(&self, index: usize) -> Self {
        let mut path = self.0.clone();
        path.push(field_numbers::NESTED_ENUM_TYPE);
        path.push(index as i32);
        Self(path)
    }

    /// Extend this path to a oneof declaration.
    pub fn oneof(&self, index: usize) -> Self {
        let mut path = self.0.clone();
        path.push(field_numbers::ONEOF_DECL);
        path.push(index as i32);
        Self(path)
    }

    /// Extend this path to an enum value.
    pub fn enum_value(&self, index: usize) -> Self {
        let mut path = self.0.clone();
        path.push(field_numbers::ENUM_VALUE);
        path.push(index as i32);
        Self(path)
    }
}

impl From<Vec<i32>> for DescriptorPath {
    fn from(path: Vec<i32>) -> Self {
        Self(path)
    }
}

/// Stores comments extracted from SourceCodeInfo, indexed by path.
#[derive(Debug, Default)]
pub struct CommentMap {
    comments: HashMap<DescriptorPath, String>,
}

impl CommentMap {
    /// Build a CommentMap from a FileDescriptorProto's SourceCodeInfo.
    pub fn from_file(file: &FileDescriptorProto) -> Self {
        let mut comments = HashMap::new();

        if let Some(source_code_info) = &file.source_code_info {
            for location in &source_code_info.location {
                if let Some(comment) = Self::extract_comment(location) {
                    let path = DescriptorPath::from(location.path.clone());
                    comments.insert(path, comment);
                }
            }
        }

        Self { comments }
    }

    /// Get the comment for a descriptor path, if any.
    pub fn get(&self, path: &DescriptorPath) -> Option<&str> {
        self.comments.get(path).map(|s| s.as_str())
    }

    /// Extract and format comment from a Location.
    fn extract_comment(location: &Location) -> Option<String> {
        let mut parts = Vec::new();

        // Add leading detached comments (separated by blank lines)
        for detached in &location.leading_detached_comments {
            let cleaned = Self::clean_comment(detached);
            if !cleaned.is_empty() {
                parts.push(cleaned);
            }
        }

        // Add leading comments
        if let Some(leading) = &location.leading_comments {
            let cleaned = Self::clean_comment(leading);
            if !cleaned.is_empty() {
                parts.push(cleaned);
            }
        }

        // Add trailing comments
        if let Some(trailing) = &location.trailing_comments {
            let cleaned = Self::clean_comment(trailing);
            if !cleaned.is_empty() {
                parts.push(cleaned);
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    }

    /// Clean up a comment string for use in Rust doc comments.
    fn clean_comment(comment: &str) -> String {
        comment
            .lines()
            .map(|line| {
                // Remove leading whitespace and common comment prefixes
                let trimmed = line.trim();
                // Remove leading asterisks from block comments
                let cleaned = trimmed.trim_start_matches('*').trim_start();
                cleaned.to_string()
            })
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Generate doc comment tokens from a comment string.
pub fn doc_comment(comment: &str) -> proc_macro2::TokenStream {
    // Add a leading space to each line so `#[doc = " text"]` renders as `/// text`
    let lines: Vec<_> = comment.lines().map(|line| format!(" {}", line)).collect();
    quote::quote! {
        #(#[doc = #lines])*
    }
}
