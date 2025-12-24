//! Tests for IsProtoDefault trait implementations.

use protomon::codec::IsProtoDefault;

#[test]
fn test_fixed_array_all_zeros_is_default() {
    // Empty array
    let arr: [u8; 0] = [];
    assert!(arr.is_proto_default());

    // Small arrays with all zeros
    assert!([0u8; 1].is_proto_default());
    assert!([0u8; 4].is_proto_default());
    assert!([0u8; 8].is_proto_default());
    assert!([0u8; 16].is_proto_default());
    assert!([0u8; 32].is_proto_default());
    assert!([0u8; 64].is_proto_default());
    assert!([0u8; 128].is_proto_default());
    assert!([0u8; 256].is_proto_default());
}

#[test]
fn test_fixed_array_first_byte_non_zero() {
    let mut arr = [0u8; 16];
    arr[0] = 1;
    assert!(!arr.is_proto_default());

    let mut arr = [0u8; 32];
    arr[0] = 255;
    assert!(!arr.is_proto_default());
}

#[test]
fn test_fixed_array_last_byte_non_zero() {
    let mut arr = [0u8; 16];
    arr[15] = 1;
    assert!(!arr.is_proto_default());

    let mut arr = [0u8; 32];
    arr[31] = 255;
    assert!(!arr.is_proto_default());
}

#[test]
fn test_fixed_array_middle_byte_non_zero() {
    let mut arr = [0u8; 16];
    arr[8] = 42;
    assert!(!arr.is_proto_default());

    let mut arr = [0u8; 64];
    arr[32] = 1;
    assert!(!arr.is_proto_default());
}

#[test]
fn test_fixed_array_all_non_zero() {
    assert!(![0xFFu8; 16].is_proto_default());
    assert!(![1u8; 32].is_proto_default());
    assert!(![42u8; 8].is_proto_default());
}

#[test]
fn test_single_byte_array() {
    assert!([0u8; 1].is_proto_default());
    assert!(![1u8; 1].is_proto_default());
    assert!(![255u8; 1].is_proto_default());
}
