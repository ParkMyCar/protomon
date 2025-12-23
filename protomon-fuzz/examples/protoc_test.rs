//! Example: Generate a test case and verify with protoc.
//!
//! Run with: cargo run -p protomon-fuzz --example protoc_test

use protomon_fuzz::{
    FieldCardinality, FieldDescriptor, FieldType, FieldValue, MessageDescriptor, MessageValue,
    ProtobufSyntax, ScalarType, ScalarValue, Schema,
};
use std::fs;
use std::io::Write;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temp directory for our test files
    let test_dir = std::env::temp_dir().join("protomon_fuzz_test");
    fs::create_dir_all(&test_dir)?;
    println!("Test directory: {}", test_dir.display());

    // Create a simple schema
    let schema = Schema {
        package: "fuzztest".to_string(),
        syntax: ProtobufSyntax::Proto3,
        messages: vec![MessageDescriptor {
            name: "TestMessage".to_string(),
            fields: vec![
                FieldDescriptor {
                    name: "id".to_string(),
                    number: 1,
                    field_type: FieldType::Scalar(ScalarType::Int32),
                    cardinality: FieldCardinality::Singular,
                },
                FieldDescriptor {
                    name: "name".to_string(),
                    number: 2,
                    field_type: FieldType::Scalar(ScalarType::String),
                    cardinality: FieldCardinality::Singular,
                },
                FieldDescriptor {
                    name: "score".to_string(),
                    number: 3,
                    field_type: FieldType::Scalar(ScalarType::Double),
                    cardinality: FieldCardinality::Singular,
                },
                FieldDescriptor {
                    name: "active".to_string(),
                    number: 4,
                    field_type: FieldType::Scalar(ScalarType::Bool),
                    cardinality: FieldCardinality::Singular,
                },
                FieldDescriptor {
                    name: "tags".to_string(),
                    number: 5,
                    field_type: FieldType::Scalar(ScalarType::String),
                    cardinality: FieldCardinality::Repeated,
                },
                FieldDescriptor {
                    name: "data".to_string(),
                    number: 6,
                    field_type: FieldType::Scalar(ScalarType::Bytes),
                    cardinality: FieldCardinality::Singular,
                },
            ],
            nested_messages: vec![],
        }],
    };

    // Create test values
    let mut msg = MessageValue::new();
    msg.fields.insert("id".to_string(), FieldValue::Scalar(ScalarValue::Int32(42)));
    msg.fields.insert("name".to_string(), FieldValue::Scalar(ScalarValue::String("Hello, Protobuf!".to_string())));
    msg.fields.insert("score".to_string(), FieldValue::Scalar(ScalarValue::Double(3.14159)));
    msg.fields.insert("active".to_string(), FieldValue::Scalar(ScalarValue::Bool(true)));
    msg.fields.insert(
        "tags".to_string(),
        FieldValue::Repeated(vec![
            FieldValue::Scalar(ScalarValue::String("rust".to_string())),
            FieldValue::Scalar(ScalarValue::String("protobuf".to_string())),
            FieldValue::Scalar(ScalarValue::String("fuzzing".to_string())),
        ]),
    );
    msg.fields.insert("data".to_string(), FieldValue::Scalar(ScalarValue::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF])));

    // Write .proto file
    let proto_path = test_dir.join("test.proto");
    let proto_content = schema.to_proto();
    fs::write(&proto_path, &proto_content)?;
    println!("\n=== Generated .proto ===\n{}", proto_content);

    // Write text format file (for protoc --encode)
    let text_path = test_dir.join("test.textproto");
    let text_content = msg.to_text_format();
    fs::write(&text_path, &text_content)?;
    println!("=== Generated Text Format ===\n{}\n", text_content);

    // Also write JSON for reference
    let json_content = msg.to_json_pretty();
    println!("=== Generated JSON (for reference) ===\n{}", json_content);

    // Use protoc to encode text format to binary
    let binary_path = test_dir.join("test.bin");

    // protoc --encode reads text format from stdin
    let mut child = Command::new("protoc")
        .arg(format!("--proto_path={}", test_dir.display()))
        .arg(format!("--encode=fuzztest.TestMessage"))
        .arg(&proto_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Write text format to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text_content.as_bytes())?;
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        eprintln!("protoc --encode failed!");
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Err("protoc failed".into());
    }

    let binary_data = output.stdout;
    fs::write(&binary_path, &binary_data)?;

    println!("\n=== Encoded binary ({} bytes) ===", binary_data.len());
    println!("Hex: {}", hex_dump(&binary_data));

    // Now decode it back with protoc to verify
    let mut decode_child = Command::new("protoc")
        .arg(format!("--proto_path={}", test_dir.display()))
        .arg(format!("--decode=fuzztest.TestMessage"))
        .arg(&proto_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = decode_child.stdin.take() {
        stdin.write_all(&binary_data)?;
    }

    let decode_output = decode_child.wait_with_output()?;

    if !decode_output.status.success() {
        eprintln!("protoc --decode failed!");
        eprintln!("stderr: {}", String::from_utf8_lossy(&decode_output.stderr));
        return Err("protoc decode failed".into());
    }

    println!("\n=== Decoded (protoc text format) ===");
    println!("{}", String::from_utf8_lossy(&decode_output.stdout));

    println!("=== Test Passed! ===");
    println!("Binary file written to: {}", binary_path.display());
    println!("\nNext step: decode this binary with protomon and verify values match!");

    Ok(())
}

fn hex_dump(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ")
}
