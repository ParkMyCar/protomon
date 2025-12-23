//! Recursive type detection for automatic boxing.
//!
//! Protobuf messages can have recursive definitions, either directly:
//! ```protobuf
//! message Node {
//!   optional Node child = 1;
//! }
//! ```
//!
//! Or indirectly:
//! ```protobuf
//! message A {
//!   optional B b = 1;
//! }
//! message B {
//!   optional A a = 1;
//! }
//! ```
//!
//! In Rust, recursive types must use indirection (Box, Rc, etc.) to have
//! a known size at compile time. This module detects such cycles and marks
//! fields that need to be boxed.

use std::collections::{HashMap, HashSet};

use crate::descriptor::{DescriptorProto, FileDescriptorSet, Type};

/// A field that needs to be boxed to break a recursive cycle.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecursiveField {
    /// Fully-qualified name of the containing message (e.g., ".mypackage.Node").
    pub message_fqn: String,
    /// Name of the field that needs boxing.
    pub field_name: String,
}

/// Analyze a FileDescriptorSet for recursive types and return fields that need boxing.
pub fn find_recursive_fields(fds: &FileDescriptorSet) -> HashSet<RecursiveField> {
    let mut result = HashSet::new();

    // Build the type graph: message_fqn -> list of (field_name, referenced_message_fqn)
    let mut graph: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for file in &fds.file {
        let package = file.package.as_deref().unwrap_or("");
        let prefix = if package.is_empty() {
            ".".to_string()
        } else {
            format!(".{}.", package)
        };

        for message in &file.message_type {
            collect_message_edges(&mut graph, &prefix, message);
        }
    }

    // For each message, check if it's part of a cycle
    for message_fqn in graph.keys() {
        find_cycles_from(&graph, message_fqn, &mut result);
    }

    result
}

/// Recursively collect edges from a message and its nested messages.
fn collect_message_edges(
    graph: &mut HashMap<String, Vec<(String, String)>>,
    prefix: &str,
    message: &DescriptorProto,
) {
    let name = match &message.name {
        Some(n) => n,
        None => return,
    };

    // Construct the fully-qualified name: prefix already ends with '.' so just append name
    let message_fqn = format!("{}{}", prefix, name);
    let mut edges = Vec::new();

    // Collect message-type field references
    for field in &message.field {
        let field_name = match &field.name {
            Some(n) => n.clone(),
            None => continue,
        };

        // Only message types can create cycles
        if field.field_type() != Some(Type::Message) {
            continue;
        }

        // Get the referenced type
        if let Some(type_name) = &field.type_name {
            edges.push((field_name, type_name.clone()));
        }
    }

    graph.insert(message_fqn.clone(), edges);

    // Process nested messages
    let nested_prefix = format!("{}.", message_fqn);
    for nested in &message.nested_type {
        // Skip map entry types
        if nested
            .options
            .as_ref()
            .and_then(|o| o.map_entry)
            .unwrap_or(false)
        {
            continue;
        }
        collect_message_edges(graph, &nested_prefix, nested);
    }
}

/// Find cycles starting from a message and mark fields that need boxing.
fn find_cycles_from(
    graph: &HashMap<String, Vec<(String, String)>>,
    start: &str,
    result: &mut HashSet<RecursiveField>,
) {
    // Use DFS to find if there's a path from start back to start
    // Track nodes currently in the DFS path (not globally visited)

    let mut in_path = HashSet::new();
    in_path.insert(start.to_string());

    dfs_find_cycles(graph, start, start, &mut in_path, result);
}

/// DFS to find cycles. `in_path` tracks nodes in the current DFS path.
fn dfs_find_cycles(
    graph: &HashMap<String, Vec<(String, String)>>,
    current: &str,
    target: &str,
    in_path: &mut HashSet<String>,
    result: &mut HashSet<RecursiveField>,
) {
    let edges = match graph.get(current) {
        Some(e) => e,
        None => return,
    };

    for (field_name, referenced_type) in edges {
        // Check if this edge completes a cycle back to target
        if referenced_type == target {
            // Found a cycle! This field creates a back-edge to target.
            result.insert(RecursiveField {
                message_fqn: current.to_string(),
                field_name: field_name.clone(),
            });
            continue;
        }

        // Skip if already in current path (avoid infinite loops in complex cycles)
        if in_path.contains(referenced_type) {
            continue;
        }

        // Continue DFS
        in_path.insert(referenced_type.clone());
        dfs_find_cycles(graph, referenced_type, target, in_path, result);
        in_path.remove(referenced_type);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{FieldDescriptorProto, FileDescriptorProto};

    fn make_message_field(name: &str, type_name: &str) -> FieldDescriptorProto {
        FieldDescriptorProto {
            name: Some(name.to_string()),
            number: Some(1),
            label: Some(1), // OPTIONAL
            r#type: Some(11), // TYPE_MESSAGE
            type_name: Some(type_name.to_string()),
            ..Default::default()
        }
    }

    fn make_message(name: &str, fields: Vec<FieldDescriptorProto>) -> DescriptorProto {
        DescriptorProto {
            name: Some(name.to_string()),
            field: fields,
            ..Default::default()
        }
    }

    #[test]
    fn test_direct_recursion() {
        // message Node { optional Node child = 1; }
        let fds = FileDescriptorSet {
            file: vec![FileDescriptorProto {
                name: Some("test.proto".to_string()),
                package: Some("test".to_string()),
                message_type: vec![make_message(
                    "Node",
                    vec![make_message_field("child", ".test.Node")],
                )],
                ..Default::default()
            }],
        };

        let recursive = find_recursive_fields(&fds);

        assert!(recursive.contains(&RecursiveField {
            message_fqn: ".test.Node".to_string(),
            field_name: "child".to_string(),
        }));
    }

    #[test]
    fn test_indirect_recursion() {
        // message A { optional B b = 1; }
        // message B { optional A a = 1; }
        let fds = FileDescriptorSet {
            file: vec![FileDescriptorProto {
                name: Some("test.proto".to_string()),
                package: Some("test".to_string()),
                message_type: vec![
                    make_message("A", vec![make_message_field("b", ".test.B")]),
                    make_message("B", vec![make_message_field("a", ".test.A")]),
                ],
                ..Default::default()
            }],
        };

        let recursive = find_recursive_fields(&fds);

        // At least one field in the cycle should be marked
        let has_a_b = recursive.contains(&RecursiveField {
            message_fqn: ".test.A".to_string(),
            field_name: "b".to_string(),
        });
        let has_b_a = recursive.contains(&RecursiveField {
            message_fqn: ".test.B".to_string(),
            field_name: "a".to_string(),
        });

        assert!(
            has_a_b || has_b_a,
            "At least one field in the cycle should be boxed"
        );
    }

    #[test]
    fn test_no_recursion() {
        // message A { optional B b = 1; }
        // message B { optional int32 x = 1; }
        let fds = FileDescriptorSet {
            file: vec![FileDescriptorProto {
                name: Some("test.proto".to_string()),
                package: Some("test".to_string()),
                message_type: vec![
                    make_message("A", vec![make_message_field("b", ".test.B")]),
                    make_message(
                        "B",
                        vec![FieldDescriptorProto {
                            name: Some("x".to_string()),
                            number: Some(1),
                            r#type: Some(5), // TYPE_INT32
                            ..Default::default()
                        }],
                    ),
                ],
                ..Default::default()
            }],
        };

        let recursive = find_recursive_fields(&fds);
        assert!(recursive.is_empty(), "No fields should need boxing");
    }
}
