//! Snapshot tests for the derive macros.

use crate::{impl_proto_message, impl_proto_oneof};
use proc_macro2::TokenStream as TokenStream2;
use syn::{parse_quote, DeriveInput};

/// Format generated tokens as pretty Rust code for snapshots.
fn format_tokens(tokens: TokenStream2) -> String {
    let file = syn::parse_file(&tokens.to_string()).expect("generated invalid syntax");
    prettyplease::unparse(&file)
}

#[test]
fn test_simple_message() {
    let input: DeriveInput = parse_quote! {
        struct Person {
            #[proto(tag = 1)]
            name: String,
            #[proto(tag = 2)]
            id: i32,
        }
    };
    let output = impl_proto_message(&input).expect("derive failed");
    insta::assert_snapshot!(format_tokens(output));
}

#[test]
fn test_message_with_optional() {
    let input: DeriveInput = parse_quote! {
        struct Message {
            #[proto(tag = 1)]
            required_field: i32,
            #[proto(tag = 2, optional)]
            optional_field: Option<String>,
        }
    };
    let output = impl_proto_message(&input).expect("derive failed");
    insta::assert_snapshot!(format_tokens(output));
}

#[test]
fn test_message_with_repeated() {
    let input: DeriveInput = parse_quote! {
        struct Message {
            #[proto(tag = 1)]
            name: String,
            #[proto(tag = 2, repeated)]
            values: Vec<i32>,
        }
    };
    let output = impl_proto_message(&input).expect("derive failed");
    insta::assert_snapshot!(format_tokens(output));
}

#[test]
fn test_message_with_map() {
    let input: DeriveInput = parse_quote! {
        struct Message {
            #[proto(tag = 1, map)]
            entries: BTreeMap<String, i32>,
        }
    };
    let output = impl_proto_message(&input).expect("derive failed");
    insta::assert_snapshot!(format_tokens(output));
}

#[test]
fn test_message_with_oneof() {
    let input: DeriveInput = parse_quote! {
        struct Message {
            #[proto(tag = 1)]
            id: i32,
            #[proto(oneof, tags = "2, 3, 4")]
            payload: Option<Payload>,
        }
    };
    let output = impl_proto_message(&input).expect("derive failed");
    insta::assert_snapshot!(format_tokens(output));
}

#[test]
fn test_message_with_required_oneof() {
    let input: DeriveInput = parse_quote! {
        struct Message {
            #[proto(tag = 1)]
            id: i32,
            #[proto(oneof, tags = "2, 3", required)]
            payload: Payload,
        }
    };
    let output = impl_proto_message(&input).expect("derive failed");
    insta::assert_snapshot!(format_tokens(output));
}

#[test]
fn test_message_with_unknown_fields() {
    let input: DeriveInput = parse_quote! {
        struct Message {
            #[proto(tag = 1)]
            known_field: i32,
            #[proto(unknown)]
            unknown_fields: bytes::Bytes,
        }
    };
    let output = impl_proto_message(&input).expect("derive failed");
    insta::assert_snapshot!(format_tokens(output));
}

#[test]
fn test_oneof_enum() {
    let input: DeriveInput = parse_quote! {
        enum Payload {
            #[proto(tag = 1)]
            IntValue(i32),
            #[proto(tag = 2)]
            StringValue(String),
            #[proto(tag = 3)]
            Nested(Box<NestedMessage>),
        }
    };
    let output = impl_proto_oneof(&input).expect("derive failed");
    insta::assert_snapshot!(format_tokens(output));
}
