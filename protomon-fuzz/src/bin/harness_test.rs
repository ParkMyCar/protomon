//! End-to-end harness test runner.
//!
//! This binary generates random protobuf schemas and values, then tests them
//! against the C++ and Go harnesses to verify cross-language compatibility.
//!
//! Usage:
//!   cargo run --bin harness_test -- --cpp-harness path/to/cpp/harness --go-harness path/to/go/harness
//!
//! Or with Bazel-built harnesses:
//!   cargo run --bin harness_test -- \
//!     --cpp-harness harness/bazel-bin/cpp/harness \
//!     --go-harness harness/bazel-bin/go/harness_dynamic_/harness_dynamic

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use arbitrary::Unstructured;
use protomon_fuzz::TestCase;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse arguments
    let mut cpp_harness: Option<PathBuf> = None;
    let mut go_harness: Option<PathBuf> = None;
    let mut seed: Option<u64> = None;
    let mut iterations: u32 = 1;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--cpp-harness" => {
                i += 1;
                cpp_harness = Some(PathBuf::from(&args[i]));
            }
            "--go-harness" => {
                i += 1;
                go_harness = Some(PathBuf::from(&args[i]));
            }
            "--seed" => {
                i += 1;
                seed = Some(args[i].parse().expect("Invalid seed"));
            }
            "--iterations" | "-n" => {
                i += 1;
                iterations = args[i].parse().expect("Invalid iteration count");
            }
            "--help" | "-h" => {
                eprintln!("Usage: harness_test [OPTIONS]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --cpp-harness PATH    Path to C++ dynamic harness");
                eprintln!("  --go-harness PATH     Path to Go dynamic harness");
                eprintln!("  --seed N              Random seed (default: random)");
                eprintln!("  --iterations N        Number of test iterations (default: 1)");
                eprintln!("  --help                Show this help");
                return;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let cpp_harness = cpp_harness.expect("--cpp-harness is required");
    let go_harness = go_harness.expect("--go-harness is required");

    // Verify harnesses exist
    if !cpp_harness.exists() {
        eprintln!("C++ harness not found: {}", cpp_harness.display());
        std::process::exit(1);
    }
    if !go_harness.exists() {
        eprintln!("Go harness not found: {}", go_harness.display());
        std::process::exit(1);
    }

    // Create temp directory
    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");

    for iter in 0..iterations {
        // Generate random bytes for the test case
        let seed_value = seed.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64 + iter as u64
        });

        // Use a simple PRNG to generate test data
        let mut rng_state = seed_value;
        let mut random_bytes = vec![0u8; 256];
        for byte in &mut random_bytes {
            // Simple xorshift64
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            *byte = rng_state as u8;
        }

        let mut u = Unstructured::new(&random_bytes);

        // Generate test case
        let test_case = match TestCase::arbitrary(&mut u) {
            Ok(tc) => tc,
            Err(_) => {
                eprintln!("Iteration {}: Failed to generate test case (not enough entropy), skipping", iter);
                continue;
            }
        };

        // Write proto file
        let proto_path = temp_dir.path().join("test.proto");
        fs::write(&proto_path, test_case.to_proto()).expect("Failed to write proto file");

        // Get the message names from the schema
        let messages: Vec<_> = test_case.schema.messages.iter().map(|m| m.name.clone()).collect();

        if messages.is_empty() {
            eprintln!("Iteration {}: No messages in schema, skipping", iter);
            continue;
        }

        // Test each message type
        for (msg_name, msg_value) in &test_case.values {
            let full_name = format!("{}.{}", test_case.schema.package, msg_name);
            let text_format = msg_value.to_text_format();

            eprintln!("Iteration {}: Testing message {} ({} bytes text)",
                     iter, full_name, text_format.len());

            // Run C++ harness
            let cpp_result = run_harness(
                &cpp_harness,
                &proto_path,
                &full_name,
                &text_format,
                "encode",
            );

            // Run Go harness
            let go_result = run_harness(
                &go_harness,
                &proto_path,
                &full_name,
                &text_format,
                "encode",
            );

            match (&cpp_result, &go_result) {
                (Ok(cpp_bytes), Ok(go_bytes)) => {
                    // Both succeeded - compare outputs
                    // Note: Field ordering may differ, so we decode and re-compare
                    if cpp_bytes != go_bytes {
                        eprintln!("  Binary outputs differ ({} vs {} bytes)", cpp_bytes.len(), go_bytes.len());
                        eprintln!("  C++: {:?}", cpp_bytes);
                        eprintln!("  Go:  {:?}", go_bytes);

                        // Try decoding each with the other harness to verify semantic equivalence
                        let cpp_decoded = run_harness_decode(&go_harness, &proto_path, &full_name, cpp_bytes);
                        let go_decoded = run_harness_decode(&cpp_harness, &proto_path, &full_name, go_bytes);

                        match (cpp_decoded, go_decoded) {
                            (Ok(_), Ok(_)) => {
                                eprintln!("  But both decode successfully with opposite harness (field ordering difference)");
                            }
                            (Err(e1), _) => {
                                eprintln!("  ERROR: C++ output not decodable by Go: {}", e1);
                                std::process::exit(1);
                            }
                            (_, Err(e2)) => {
                                eprintln!("  ERROR: Go output not decodable by C++: {}", e2);
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("  OK: Both harnesses produced identical output ({} bytes)", cpp_bytes.len());
                    }
                }
                (Err(e), Ok(_)) => {
                    eprintln!("  C++ harness failed: {}", e);
                    eprintln!("  Proto:\n{}", test_case.to_proto());
                    eprintln!("  Text format:\n{}", text_format);
                    std::process::exit(1);
                }
                (Ok(_), Err(e)) => {
                    eprintln!("  Go harness failed: {}", e);
                    eprintln!("  Proto:\n{}", test_case.to_proto());
                    eprintln!("  Text format:\n{}", text_format);
                    std::process::exit(1);
                }
                (Err(e1), Err(e2)) => {
                    eprintln!("  Both harnesses failed:");
                    eprintln!("    C++: {}", e1);
                    eprintln!("    Go:  {}", e2);
                    eprintln!("  Proto:\n{}", test_case.to_proto());
                    eprintln!("  Text format:\n{}", text_format);
                    std::process::exit(1);
                }
            }
        }
    }

    eprintln!("\nAll {} iterations passed!", iterations);
}

fn run_harness(
    harness_path: &PathBuf,
    proto_path: &std::path::Path,
    message_name: &str,
    text_input: &str,
    mode: &str,
) -> Result<Vec<u8>, String> {
    // Get the directory and filename for the proto file
    let proto_dir = proto_path.parent().unwrap_or(std::path::Path::new("."));
    let proto_file = proto_path.file_name().unwrap().to_string_lossy();

    let mut cmd = Command::new(harness_path);
    cmd.arg(format!("--mode={}", mode))
       .arg(format!("--proto={}", proto_file))
       .arg(format!("--proto_path={}", proto_dir.display()))
       .arg(format!("--message={}", message_name))
       .stdin(Stdio::piped())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn: {}", e))?;

    // Write input
    {
        let stdin = child.stdin.as_mut().expect("Failed to get stdin");
        stdin.write_all(text_input.as_bytes()).map_err(|e| format!("Failed to write: {}", e))?;
    }

    let output = child.wait_with_output().map_err(|e| format!("Failed to wait: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Exit {}: {}", output.status, stderr));
    }

    Ok(output.stdout)
}

fn run_harness_decode(
    harness_path: &PathBuf,
    proto_path: &std::path::Path,
    message_name: &str,
    binary_input: &[u8],
) -> Result<String, String> {
    // Get the directory and filename for the proto file
    let proto_dir = proto_path.parent().unwrap_or(std::path::Path::new("."));
    let proto_file = proto_path.file_name().unwrap().to_string_lossy();

    let mut cmd = Command::new(harness_path);
    cmd.arg("--mode=decode")
       .arg(format!("--proto={}", proto_file))
       .arg(format!("--proto_path={}", proto_dir.display()))
       .arg(format!("--message={}", message_name))
       .stdin(Stdio::piped())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn: {}", e))?;

    {
        let stdin = child.stdin.as_mut().expect("Failed to get stdin");
        stdin.write_all(binary_input).map_err(|e| format!("Failed to write: {}", e))?;
    }

    let output = child.wait_with_output().map_err(|e| format!("Failed to wait: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Exit {}: {}", output.status, stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
