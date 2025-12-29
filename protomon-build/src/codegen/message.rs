//! Message struct code generation.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::context::{to_rust_field_name, GenerationContext};
use crate::descriptor::DescriptorProto;
use crate::Error;

use super::comments::{doc_comment, CommentMap, DescriptorPath};
use super::enumeration::generate_enum;
use super::field::generate_field;
use super::oneof::{collect_oneofs, generate_oneof_enum, generate_oneof_field};

/// Generate Rust struct for a proto message.
pub fn generate_message(
    ctx: &GenerationContext,
    parent_path: &str,
    message: &DescriptorProto,
    is_proto3: bool,
    comments: &CommentMap,
    msg_path: &DescriptorPath,
) -> Result<TokenStream, Error> {
    let name = message.name.as_ref().ok_or(Error::MissingName)?;
    let struct_name = format_ident!("{}", name);
    let full_path = if parent_path.is_empty() {
        format!(".{}", name)
    } else {
        format!("{}.{}", parent_path, name)
    };

    // Generate doc comment for the struct
    let struct_doc = comments
        .get(msg_path)
        .map(|c| doc_comment(c))
        .unwrap_or_default();

    // Collect oneofs (excluding proto3 optional synthetic oneofs)
    let oneofs = collect_oneofs(message);

    // Build a set of field indices that belong to real oneofs
    let oneof_field_indices: std::collections::HashSet<i32> = oneofs
        .iter()
        .flat_map(|o| o.fields.iter().filter_map(|f| f.number))
        .collect();

    // Build field index to original position map for comment lookup
    let field_indices: std::collections::HashMap<i32, usize> = message
        .field
        .iter()
        .enumerate()
        .filter_map(|(i, f)| f.number.map(|n| (n, i)))
        .collect();

    // Generate regular fields (excluding oneof fields)
    // Note: In proto3, optional fields use synthetic oneofs for presence tracking.
    // We include fields with oneof_index if they have proto3_optional set to true.
    let fields: Vec<TokenStream> = message
        .field
        .iter()
        .filter(|f| {
            let field_num = f.number.unwrap_or(-1);
            // Include if:
            // - No oneof_index, OR
            // - It's a proto3 optional field (synthetic oneof), OR
            // - It's NOT part of a real oneof (checked via field number)
            f.oneof_index.is_none()
                || f.proto3_optional.unwrap_or(false)
                || !oneof_field_indices.contains(&field_num)
        })
        .map(|f| {
            let field_index = f.number.and_then(|n| field_indices.get(&n).copied());
            generate_field(ctx, &full_path, f, is_proto3, comments, msg_path, field_index)
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Generate oneof fields
    let oneof_fields: Vec<TokenStream> = oneofs
        .iter()
        .map(|o| generate_oneof_field(name, o, comments, msg_path))
        .collect::<Result<Vec<_>, _>>()?;

    // Check if we should add an _unknown field for preserving unknown fields
    let unknown_field = if message
        .options
        .as_ref()
        .map(|o| o.should_preserve_unknown())
        .unwrap_or(false)
    {
        quote! {
            /// Unknown fields for round-trip compatibility.
            /// This field stores any unrecognized protobuf fields encountered during decoding
            /// and re-serializes them during encoding.
            #[proto(unknown)]
            pub _unknown: bytes::Bytes,
        }
    } else {
        quote!()
    };

    // Generate nested types
    let mut nested = TokenStream::new();

    // Oneof enums (generated in the nested module)
    for oneof in &oneofs {
        let oneof_enum = generate_oneof_enum(ctx, oneof, is_proto3)?;
        nested.extend(oneof_enum);
    }

    // Nested enums
    for (enum_index, enum_type) in message.enum_type.iter().enumerate() {
        let enum_path = msg_path.nested_enum(enum_index);
        let enum_tokens = generate_enum(&full_path, enum_type, comments, &enum_path)?;
        nested.extend(enum_tokens);
    }

    // Nested messages (with map entry counter for proper indexing)
    let mut nested_msg_index = 0;
    for nested_msg in &message.nested_type {
        // Skip map entry types (synthetic messages for map fields)
        if nested_msg
            .options
            .as_ref()
            .and_then(|o| o.map_entry)
            .unwrap_or(false)
        {
            nested_msg_index += 1;
            continue;
        }
        let nested_path = msg_path.nested_message(nested_msg_index);
        let msg_tokens =
            generate_message(ctx, &full_path, nested_msg, is_proto3, comments, &nested_path)?;
        nested.extend(msg_tokens);
        nested_msg_index += 1;
    }

    let nested_mod = if nested.is_empty() {
        quote!()
    } else {
        let mod_name = format_ident!("{}", to_rust_field_name(name));
        quote! {
            pub mod #mod_name {
                use super::*;
                #nested
            }
        }
    };

    Ok(quote! {
        #struct_doc
        #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
        pub struct #struct_name {
            #(#fields)*
            #(#oneof_fields)*
            #unknown_field
        }

        #nested_mod
    })
}
