//! Message struct code generation.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::context::{to_rust_field_name, GenerationContext};
use crate::descriptor::DescriptorProto;
use crate::Error;

use super::enumeration::generate_enum;
use super::field::generate_field;

/// Generate Rust struct for a proto message.
pub fn generate_message(
    ctx: &GenerationContext,
    parent_path: &str,
    message: &DescriptorProto,
    is_proto3: bool,
) -> Result<TokenStream, Error> {
    let name = message.name.as_ref().ok_or(Error::MissingName)?;
    let struct_name = format_ident!("{}", name);
    let full_path = if parent_path.is_empty() {
        format!(".{}", name)
    } else {
        format!("{}.{}", parent_path, name)
    };

    // Generate fields
    // Note: In proto3, optional fields use synthetic oneofs for presence tracking.
    // We include fields with oneof_index if they have proto3_optional set to true.
    // Real oneof fields (not proto3 optional) are skipped for now.
    let fields: Vec<TokenStream> = message
        .field
        .iter()
        .filter(|f| {
            // Include if no oneof_index, or if it's a proto3 optional field
            f.oneof_index.is_none() || f.proto3_optional.unwrap_or(false)
        })
        .map(|f| generate_field(ctx, &full_path, f, is_proto3))
        .collect::<Result<Vec<_>, _>>()?;

    // Generate nested types
    let mut nested = TokenStream::new();

    // Nested enums
    for enum_type in &message.enum_type {
        let enum_tokens = generate_enum(&full_path, enum_type)?;
        nested.extend(enum_tokens);
    }

    // Nested messages
    for nested_msg in &message.nested_type {
        // Skip map entry types (synthetic messages for map fields)
        if nested_msg
            .options
            .as_ref()
            .and_then(|o| o.map_entry)
            .unwrap_or(false)
        {
            continue;
        }
        let msg_tokens = generate_message(ctx, &full_path, nested_msg, is_proto3)?;
        nested.extend(msg_tokens);
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
        #[derive(Debug, Clone, Default, protomon::ProtoMessage)]
        pub struct #struct_name {
            #(#fields)*
        }

        #nested_mod
    })
}
