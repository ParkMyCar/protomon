//! Field code generation.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::context::{to_rust_field_name, GenerationContext};
use crate::descriptor::{FieldDescriptorProto, Label, Type};
use crate::Error;

use super::types::{build_full_type, proto_type_to_rust, map_key_type_to_rust, scalar_type_to_rust};

/// Generate a struct field with #[proto(...)] attribute.
pub fn generate_field(
    ctx: &GenerationContext,
    parent_path: &str,
    field: &FieldDescriptorProto,
    is_proto3: bool,
) -> Result<TokenStream, Error> {
    let name = field.name.as_ref().ok_or(Error::MissingName)?;
    let tag = field.number.ok_or(Error::MissingFieldNumber)?;
    let field_name = format_ident!("{}", to_rust_field_name(name));

    let proto_type = field.field_type().ok_or(Error::InvalidFieldType(
        field.r#type.unwrap_or(-1),
    ))?;
    let label = field.label();
    let proto3_optional = field.proto3_optional.unwrap_or(false);

    // Check if this is a map field (repeated message where message is a map entry)
    if label == Label::Repeated && proto_type == Type::Message {
        if let Some(type_name) = field.type_name.as_deref() {
            if let Some(map_entry) = ctx.get_map_entry(type_name) {
                return generate_map_field(ctx, field, tag, &field_name, map_entry);
            }
        }
    }

    // Check if this field needs auto-boxing due to recursive type cycle
    let auto_box = ctx.is_recursive_field(parent_path, name);

    let rust_type = proto_type_to_rust(
        ctx,
        proto_type,
        field.type_name.as_deref(),
        label,
        is_proto3,
        proto3_optional,
        field.options.as_ref(),
        auto_box,
    )?;

    // Build the full Rust type with wrappers
    let full_type = build_full_type(&rust_type);

    // Build #[proto(...)] attribute
    let proto_attr = build_proto_attr(tag, &rust_type);

    Ok(quote! {
        #proto_attr
        pub #field_name: #full_type,
    })
}

/// Generate a map field.
fn generate_map_field(
    ctx: &GenerationContext,
    field: &FieldDescriptorProto,
    tag: i32,
    field_name: &proc_macro2::Ident,
    map_entry: &crate::context::MapEntryInfo,
) -> Result<TokenStream, Error> {
    let tag_lit = proc_macro2::Literal::i32_unsuffixed(tag);

    // Get key type
    let key_type = map_entry.key_field.field_type().ok_or(Error::InvalidFieldType(
        map_entry.key_field.r#type.unwrap_or(-1),
    ))?;
    let key_rust_type = map_key_type_to_rust(key_type)?;

    // Get value type
    let value_type = map_entry.value_field.field_type().ok_or(Error::InvalidFieldType(
        map_entry.value_field.r#type.unwrap_or(-1),
    ))?;
    let value_rust_type = scalar_type_to_rust(
        ctx,
        value_type,
        map_entry.value_field.type_name.as_deref(),
    )?;

    // Check map_type extension to determine BTreeMap vs HashMap
    let use_hash_map = field
        .options
        .as_ref()
        .and_then(|o| o.map_type.as_deref())
        .map(|s| s == "hash")
        .unwrap_or(false);

    // Validate map_type value if present
    if let Some(map_type_str) = field.options.as_ref().and_then(|o| o.map_type.as_deref()) {
        if map_type_str != "hash" && map_type_str != "btree" {
            return Err(Error::InvalidOption(format!(
                "Invalid map_type value '{}'. Must be 'btree' or 'hash'.",
                map_type_str
            )));
        }
    }

    let map_type = if use_hash_map {
        quote!(std::collections::HashMap<#key_rust_type, #value_rust_type>)
    } else {
        quote!(alloc::collections::BTreeMap<#key_rust_type, #value_rust_type>)
    };

    Ok(quote! {
        #[proto(tag = #tag_lit, map)]
        pub #field_name: #map_type,
    })
}

/// Build the #[proto(tag = N, ...)] attribute.
fn build_proto_attr(tag: i32, rust_type: &super::types::RustType) -> TokenStream {
    let tag_lit = proc_macro2::Literal::i32_unsuffixed(tag);
    let mut parts = vec![quote!(tag = #tag_lit)];

    if rust_type.is_repeated {
        parts.push(quote!(repeated));
    }

    if rust_type.is_optional {
        parts.push(quote!(optional));
    }

    quote! {
        #[proto(#(#parts),*)]
    }
}
