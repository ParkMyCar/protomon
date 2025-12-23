//! Integration test for protomon-build.

use protomon_build::Config;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_compile_simple_proto() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .compile_protos(&["tests/proto/test.proto"], &["tests/proto/"])
        .expect("Failed to compile protos");

    // Check that output files were created
    let test_rs = out_dir.path().join("test.rs");
    assert!(test_rs.exists(), "test.rs should be generated");

    let mod_rs = out_dir.path().join("mod.rs");
    assert!(mod_rs.exists(), "mod.rs should be generated");

    // Read and verify the generated code
    let content = fs::read_to_string(&test_rs).expect("Failed to read test.rs");

    // Check for expected structures
    assert!(content.contains("pub struct Person"), "Should contain Person struct");
    assert!(content.contains("pub struct PhoneNumber"), "Should contain PhoneNumber struct");
    assert!(content.contains("pub enum PhoneType"), "Should contain PhoneType enum");

    // Check for expected fields
    assert!(content.contains("pub name:"), "Should contain name field");
    assert!(content.contains("pub id:"), "Should contain id field");
    assert!(content.contains("pub email:"), "Should contain email field");
    assert!(content.contains("pub phones:"), "Should contain phones field");

    // Check for proto attributes
    assert!(content.contains("#[proto(tag = 1)]"), "Should contain proto attribute with tag 1");
    assert!(content.contains("#[proto(tag = 2)]"), "Should contain proto attribute with tag 2");

    // Check for optional field
    assert!(content.contains("optional"), "Should have optional attribute for email");

    // Check for derive macro
    assert!(content.contains("#[derive("), "Should have derive attribute");
    assert!(content.contains("protomon::ProtoMessage"), "Should derive ProtoMessage");
}

#[test]
fn test_compile_with_extensions() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .compile_protos(
            &["tests/proto/test_extensions.proto"],
            &["tests/proto/"],
        )
        .expect("Failed to compile protos");

    let test_rs = out_dir.path().join("test_extensions.rs");
    let content = fs::read_to_string(&test_rs).expect("Failed to read test_extensions.rs");

    // Check that regular repeated uses Repeated<T>
    assert!(
        content.contains("Repeated<"),
        "Regular repeated field should use Repeated<T>"
    );

    // Check that vec extension uses Vec<T>
    assert!(
        content.contains("Vec<"),
        "Field with [(protomon.vec) = true] should use Vec<T>"
    );

    // Check that boxed extension uses Box<T>
    assert!(
        content.contains("Box<"),
        "Field with [(protomon.boxed) = true] should use Box<T>"
    );

    // Check that lazy extension uses LazyMessage<T>
    assert!(
        content.contains("LazyMessage<"),
        "Field with [(protomon.lazy) = true] should use LazyMessage<T>"
    );

    // Check that lazy_child uses LazyMessage
    assert!(
        content.contains("LazyMessage<Container>"),
        "Field with lazy option should use LazyMessage<Container>"
    );

    // Note: eager_child and other recursive Container fields are auto-boxed
    // because Container references itself. The auto-boxing feature detects
    // recursive types and adds Box<T> to break the cycle.
    assert!(
        content.contains("Option<Box<Container>>"),
        "Recursive fields should be auto-boxed with Option<Box<T>>"
    );
}

#[test]
fn test_recursive_type_detection() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .compile_protos(
            &["tests/proto/test_recursive.proto"],
            &["tests/proto/"],
        )
        .expect("Failed to compile protos");

    let test_rs = out_dir.path().join("test_recursive.rs");
    let content = fs::read_to_string(&test_rs).expect("Failed to read test_recursive.rs");

    // Direct recursion: Node.left and Node.right should be auto-boxed
    assert!(
        content.contains("pub left: Option<Box<Node>>"),
        "Direct recursive field 'left' should be auto-boxed"
    );
    assert!(
        content.contains("pub right: Option<Box<Node>>"),
        "Direct recursive field 'right' should be auto-boxed"
    );

    // Indirect recursion: TreeA.child -> TreeB and TreeB.parent -> TreeA
    // At least one field in the cycle should be boxed
    let tree_a_boxed = content.contains("pub child: Option<Box<TreeB>>");
    let tree_b_boxed = content.contains("pub parent: Option<Box<TreeA>>");
    assert!(
        tree_a_boxed || tree_b_boxed,
        "Indirect recursive cycle should have at least one boxed field"
    );

    // Non-recursive types should NOT be boxed
    assert!(
        content.contains("pub data: Option<"),
        "Non-recursive field 'data' should exist"
    );
    assert!(
        !content.contains("pub data: Option<Box<"),
        "Non-recursive field 'data' should NOT be boxed"
    );

    // Container.leaves references Leaf (non-recursive), should not be boxed
    assert!(
        content.contains("Repeated<Leaf>") || content.contains("Vec<Leaf>"),
        "Container.leaves should reference Leaf without Box"
    );
}

#[test]
fn test_enum_generation() {
    let out_dir = tempdir().expect("Failed to create temp dir");

    Config::new()
        .out_dir(out_dir.path())
        .compile_protos(&["tests/proto/test.proto"], &["tests/proto/"])
        .expect("Failed to compile protos");

    let test_rs = out_dir.path().join("test.rs");
    let content = fs::read_to_string(&test_rs).expect("Failed to read test.rs");

    // Check enum variants
    assert!(content.contains("Mobile"), "Should contain Mobile variant");
    assert!(content.contains("Home"), "Should contain Home variant");
    assert!(content.contains("Work"), "Should contain Work variant");

    // Check for from_i32 method
    assert!(content.contains("fn from_i32"), "Should have from_i32 method");

    // Check for Default impl
    assert!(content.contains("impl Default for PhoneType"), "Should implement Default for PhoneType");
}
