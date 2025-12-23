//! Generation context for type resolution.

use std::collections::{HashMap, HashSet};

use crate::codegen::{find_recursive_fields, RecursiveField};
use crate::config::Config;
use crate::descriptor::{DescriptorProto, FieldDescriptorProto, FileDescriptorSet};

/// Information about a type in the registry.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TypeInfo {
    /// The file this type is defined in.
    pub file_name: String,
    /// Whether this is a message type.
    pub is_message: bool,
    /// Whether this is an enum type.
    pub is_enum: bool,
    /// The Rust module path for this type.
    pub rust_module: String,
}

/// Information about a map entry message type.
#[derive(Debug, Clone)]
pub struct MapEntryInfo {
    /// The key field descriptor.
    pub key_field: FieldDescriptorProto,
    /// The value field descriptor.
    pub value_field: FieldDescriptorProto,
}

/// Context for code generation, including type registry.
pub struct GenerationContext<'a> {
    /// The configuration.
    pub config: &'a Config,
    /// Map from fully-qualified proto type name -> type info.
    pub type_registry: HashMap<String, TypeInfo>,
    /// Fields that need to be boxed due to recursive type cycles.
    pub recursive_fields: HashSet<RecursiveField>,
    /// Map from fully-qualified map entry type name -> map entry info.
    pub map_entries: HashMap<String, MapEntryInfo>,
}

impl<'a> GenerationContext<'a> {
    /// Create a new generation context.
    pub fn new(config: &'a Config, fds: &FileDescriptorSet) -> Self {
        let mut type_registry = HashMap::new();
        let mut map_entries = HashMap::new();

        for file in &fds.file {
            let file_name = file.name.clone().unwrap_or_default();
            let package = file.package.as_deref().unwrap_or("");
            let rust_module = package_to_module(package);

            let prefix = if package.is_empty() {
                ".".to_string()
            } else {
                format!(".{}.", package)
            };

            // Register top-level messages
            for message in &file.message_type {
                register_message(&mut type_registry, &mut map_entries, &file_name, &rust_module, &prefix, message);
            }

            // Register top-level enums
            for enum_type in &file.enum_type {
                if let Some(name) = &enum_type.name {
                    let full_name = format!("{}{}", prefix, name);
                    type_registry.insert(
                        full_name,
                        TypeInfo {
                            file_name: file_name.clone(),
                            is_message: false,
                            is_enum: true,
                            rust_module: rust_module.clone(),
                        },
                    );
                }
            }
        }

        // Detect recursive types and fields that need boxing
        let recursive_fields = find_recursive_fields(fds);

        Self {
            config,
            type_registry,
            recursive_fields,
            map_entries,
        }
    }

    /// Get map entry info if the type is a map entry.
    pub fn get_map_entry(&self, type_name: &str) -> Option<&MapEntryInfo> {
        self.map_entries.get(type_name)
    }

    /// Check if a field needs to be boxed due to recursive type cycles.
    pub fn is_recursive_field(&self, message_fqn: &str, field_name: &str) -> bool {
        self.recursive_fields.contains(&RecursiveField {
            message_fqn: message_fqn.to_string(),
            field_name: field_name.to_string(),
        })
    }

    /// Resolve a proto type name to Rust type path.
    pub fn resolve_type(&self, proto_type_name: &str) -> Option<String> {
        // Check extern paths first
        if let Some(extern_path) = self.config.extern_paths.get(proto_type_name) {
            return Some(extern_path.clone());
        }

        // Look up in type registry
        self.type_registry
            .get(proto_type_name)
            .map(|info| proto_path_to_rust_type(proto_type_name, &info.rust_module))
    }

    /// Check if a type is an enum.
    #[allow(dead_code)]
    pub fn is_enum(&self, proto_type_name: &str) -> bool {
        self.type_registry
            .get(proto_type_name)
            .map(|info| info.is_enum)
            .unwrap_or(false)
    }
}

/// Register a message and its nested types in the registry.
fn register_message(
    registry: &mut HashMap<String, TypeInfo>,
    map_entries: &mut HashMap<String, MapEntryInfo>,
    file_name: &str,
    rust_module: &str,
    prefix: &str,
    message: &DescriptorProto,
) {
    if let Some(name) = &message.name {
        // Build fully-qualified name with proper dot separators
        // prefix is like "." or ".package." or ".Parent."
        let full_name = format!("{}{}", prefix, name);

        // Check if this is a map entry type
        let is_map_entry = message
            .options
            .as_ref()
            .and_then(|o| o.map_entry)
            .unwrap_or(false);

        if is_map_entry {
            // Extract key (tag 1) and value (tag 2) fields
            let key_field = message.field.iter().find(|f| f.number == Some(1));
            let value_field = message.field.iter().find(|f| f.number == Some(2));

            if let (Some(key), Some(value)) = (key_field, value_field) {
                map_entries.insert(
                    full_name.clone(),
                    MapEntryInfo {
                        key_field: key.clone(),
                        value_field: value.clone(),
                    },
                );
            }
        }

        registry.insert(
            full_name.clone(),
            TypeInfo {
                file_name: file_name.to_string(),
                is_message: true,
                is_enum: false,
                rust_module: rust_module.to_string(),
            },
        );

        // Nested prefix includes parent name with trailing dot
        let nested_prefix = format!("{}.", full_name);

        // Register nested messages
        for nested in &message.nested_type {
            register_message(registry, map_entries, file_name, rust_module, &nested_prefix, nested);
        }

        // Register nested enums
        for enum_type in &message.enum_type {
            if let Some(enum_name) = &enum_type.name {
                let full_enum_name = format!("{}{}", nested_prefix, enum_name);
                registry.insert(
                    full_enum_name,
                    TypeInfo {
                        file_name: file_name.to_string(),
                        is_message: false,
                        is_enum: true,
                        rust_module: rust_module.to_string(),
                    },
                );
            }
        }
    }
}

/// Convert package name to Rust module name.
fn package_to_module(package: &str) -> String {
    if package.is_empty() {
        String::new()
    } else {
        package.replace('.', "_")
    }
}

/// Convert proto fully-qualified type name to Rust type path.
///
/// Examples:
/// - ".mypackage.MyMessage.NestedMessage" -> "MyMessage::NestedMessage"
/// - ".com.example.MyMessage" (with rust_module "com_example") -> "MyMessage"
pub fn proto_path_to_rust_type(proto_path: &str, rust_module: &str) -> String {
    let components: Vec<&str> = proto_path.trim_start_matches('.').split('.').collect();

    // Determine how many package components to skip
    // If rust_module is "com_example", that means 2 package levels
    let package_depth = if rust_module.is_empty() {
        0
    } else {
        rust_module.split('_').count()
    };

    let type_components: Vec<_> = components
        .into_iter()
        .skip(package_depth)
        .map(to_rust_type_name)
        .collect();

    // Fallback: if we've skipped everything, use the last component of the proto_path
    if type_components.is_empty() {
        proto_path
            .rsplit('.')
            .next()
            .map(to_rust_type_name)
            .unwrap_or_default()
    } else {
        type_components.join("::")
    }
}

/// Convert proto name to valid Rust type identifier (PascalCase).
pub fn to_rust_type_name(name: &str) -> String {
    let ident = name.to_string();
    if is_rust_keyword(&ident) {
        format!("r#{}", ident)
    } else {
        ident
    }
}

/// Convert proto field name to Rust field name (snake_case).
pub fn to_rust_field_name(name: &str) -> String {
    let snake = to_snake_case(name);
    if is_rust_keyword(&snake) {
        format!("r#{}", snake)
    } else {
        snake
    }
}

/// Convert a string to snake_case.
///
/// Handles consecutive uppercase letters correctly:
/// - "HTTPServer" -> "http_server"
/// - "myField" -> "my_field"
/// - "XMLParser" -> "xml_parser"
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            // Add underscore if:
            // 1. Not at start AND previous char was lowercase, OR
            // 2. Not at start AND previous was uppercase AND next is lowercase
            //    (handles "HTTPServer" -> "http_server")
            if i > 0 {
                let prev_lower = chars[i - 1].is_lowercase();
                let prev_upper = chars[i - 1].is_uppercase();
                let next_lower = chars.get(i + 1).map(|c| c.is_lowercase()).unwrap_or(false);
                if prev_lower || (prev_upper && next_lower) {
                    result.push('_');
                }
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

/// Check if a string is a Rust keyword.
fn is_rust_keyword(s: &str) -> bool {
    matches!(
        s,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        // Simple camelCase
        assert_eq!(to_snake_case("myField"), "my_field");
        assert_eq!(to_snake_case("firstName"), "first_name");

        // Consecutive uppercase (acronyms)
        assert_eq!(to_snake_case("HTTPServer"), "http_server");
        assert_eq!(to_snake_case("XMLParser"), "xml_parser");
        assert_eq!(to_snake_case("getHTTPResponse"), "get_http_response");

        // Already snake_case
        assert_eq!(to_snake_case("my_field"), "my_field");
        assert_eq!(to_snake_case("http_server"), "http_server");

        // All uppercase
        assert_eq!(to_snake_case("HTTP"), "http");
        assert_eq!(to_snake_case("ID"), "id");

        // Single chars
        assert_eq!(to_snake_case("A"), "a");
        assert_eq!(to_snake_case("a"), "a");

        // PascalCase
        assert_eq!(to_snake_case("MyMessage"), "my_message");
        assert_eq!(to_snake_case("PersonInfo"), "person_info");
    }

    #[test]
    fn test_to_rust_field_name_keywords() {
        // Rust keywords should be escaped
        assert_eq!(to_rust_field_name("type"), "r#type");
        assert_eq!(to_rust_field_name("match"), "r#match");
        assert_eq!(to_rust_field_name("async"), "r#async");

        // Non-keywords should pass through
        assert_eq!(to_rust_field_name("name"), "name");
        assert_eq!(to_rust_field_name("value"), "value");
    }

    #[test]
    fn test_proto_path_to_rust_type() {
        // Single-level package
        assert_eq!(
            proto_path_to_rust_type(".mypackage.MyMessage", "mypackage"),
            "MyMessage"
        );
        assert_eq!(
            proto_path_to_rust_type(".mypackage.MyMessage.Nested", "mypackage"),
            "MyMessage::Nested"
        );

        // Multi-level package
        assert_eq!(
            proto_path_to_rust_type(".com.example.MyMessage", "com_example"),
            "MyMessage"
        );
        assert_eq!(
            proto_path_to_rust_type(".com.example.api.MyMessage", "com_example_api"),
            "MyMessage"
        );

        // No package (empty rust_module)
        assert_eq!(
            proto_path_to_rust_type(".MyMessage", ""),
            "MyMessage"
        );
        assert_eq!(
            proto_path_to_rust_type(".MyMessage.Nested", ""),
            "MyMessage::Nested"
        );
    }
}
