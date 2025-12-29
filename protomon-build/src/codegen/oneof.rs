//! Oneof enum code generation.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::context::{to_rust_field_name, GenerationContext};
use crate::descriptor::{DescriptorProto, FieldDescriptorProto, OneofDescriptorProto};
use crate::Error;

use super::comments::{doc_comment, CommentMap, DescriptorPath};
use super::types::proto_type_to_rust;

/// Information about a oneof and its fields.
pub struct OneofInfo<'a> {
    /// The oneof descriptor.
    pub oneof: &'a OneofDescriptorProto,
    /// Fields that belong to this oneof.
    pub fields: Vec<&'a FieldDescriptorProto>,
    /// Whether this oneof is nullable (wrapped in Option).
    pub nullable: bool,
    /// Index of the oneof in the parent message (for comment lookup).
    pub oneof_index: usize,
}

/// Collect oneofs from a message, filtering out synthetic oneofs (proto3 optional).
pub fn collect_oneofs<'a>(message: &'a DescriptorProto) -> Vec<OneofInfo<'a>> {
    message
        .oneof_decl
        .iter()
        .enumerate()
        .filter_map(|(index, oneof)| {
            // Collect fields that belong to this oneof
            let fields: Vec<_> = message
                .field
                .iter()
                .filter(|f| {
                    f.oneof_index == Some(index as i32)
                        // Skip proto3 optional fields - they use synthetic oneofs
                        && !f.proto3_optional.unwrap_or(false)
                })
                .collect();

            // Skip empty oneofs (e.g., proto3 optional synthetic oneofs)
            if fields.is_empty() {
                return None;
            }

            // Determine if nullable based on options
            let nullable = oneof
                .options
                .as_ref()
                .map(|o| o.is_nullable())
                .unwrap_or(true);

            Some(OneofInfo {
                oneof,
                fields,
                nullable,
                oneof_index: index,
            })
        })
        .collect()
}

/// Generate a oneof enum type.
pub fn generate_oneof_enum(
    ctx: &GenerationContext,
    oneof: &OneofInfo,
    is_proto3: bool,
) -> Result<TokenStream, Error> {
    let oneof_name = oneof.oneof.name.as_ref().ok_or(Error::MissingName)?;
    let enum_name = format_ident!("{}", to_pascal_case(oneof_name));

    let mut variants = Vec::new();

    for field in &oneof.fields {
        let field_name = field.name.as_ref().ok_or(Error::MissingName)?;
        let variant_name = format_ident!("{}", to_pascal_case(field_name));
        let tag = field.number.ok_or(Error::MissingFieldNumber)? as u32;

        let proto_type = field
            .field_type()
            .ok_or_else(|| Error::DecodeError("Missing field type in oneof field".into()))?;
        let type_name = field.type_name.as_deref();

        // Get the Rust type for this field
        let rust_type = proto_type_to_rust(
            ctx,
            proto_type,
            type_name,
            field.label(),
            is_proto3,
            false, // proto3_optional is false for oneof fields
            field.options.as_ref(),
            false, // auto_box - oneofs don't auto-box
        )?;

        let base_type = &rust_type.base_type;

        // Wrap in Box if boxed option is set
        let field_type = if rust_type.is_boxed {
            quote!(Box<#base_type>)
        } else {
            quote!(#base_type)
        };

        variants.push(quote! {
            #[proto(tag = #tag)]
            #variant_name(#field_type),
        });
    }

    // Only non-nullable oneofs need Default (nullable ones use Option<T> which defaults to None)
    let default_impl = if !oneof.nullable {
        let first_variant_name = if let Some(first_field) = oneof.fields.first() {
            let field_name = first_field.name.as_ref().ok_or(Error::MissingName)?;
            format_ident!("{}", to_pascal_case(field_name))
        } else {
            return Err(Error::DecodeError(
                "Oneof must have at least one field".into(),
            ));
        };

        quote! {
            impl Default for #enum_name {
                fn default() -> Self {
                    Self::#first_variant_name(Default::default())
                }
            }
        }
    } else {
        quote!()
    };

    Ok(quote! {
        #[derive(Debug, Clone, PartialEq, protomon::ProtoOneof)]
        pub enum #enum_name {
            #(#variants)*
        }

        #default_impl
    })
}

/// Generate the field declaration for a oneof in a message struct.
pub fn generate_oneof_field(
    parent_message_name: &str,
    oneof: &OneofInfo,
    comments: &CommentMap,
    msg_path: &DescriptorPath,
) -> Result<TokenStream, Error> {
    let oneof_name = oneof.oneof.name.as_ref().ok_or(Error::MissingName)?;
    let field_name = format_ident!("{}", to_rust_field_name(oneof_name));
    let enum_name = format_ident!("{}", to_pascal_case(oneof_name));

    // Get oneof comment
    let oneof_path = msg_path.oneof(oneof.oneof_index);
    let oneof_doc = comments
        .get(&oneof_path)
        .map(|c| doc_comment(c))
        .unwrap_or_default();

    // The enum is defined in a submodule named after the parent message
    let mod_name = format_ident!("{}", to_rust_field_name(parent_message_name));
    let full_enum_type = quote!(#mod_name::#enum_name);

    // Collect all tags for the oneof attribute
    let tags: Vec<u32> = oneof
        .fields
        .iter()
        .filter_map(|f| f.number.map(|n| n as u32))
        .collect();
    let tags_str = tags
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    if oneof.nullable {
        // Nullable oneof: Option<EnumType>
        Ok(quote! {
            #oneof_doc
            #[proto(oneof, tags = #tags_str)]
            pub #field_name: Option<#full_enum_type>,
        })
    } else {
        // Non-nullable oneof: EnumType with required attribute
        Ok(quote! {
            #oneof_doc
            #[proto(oneof, tags = #tags_str, required)]
            pub #field_name: #full_enum_type,
        })
    }
}

/// Convert snake_case to PascalCase.
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("foo"), "Foo");
        assert_eq!(to_pascal_case("foo_bar"), "FooBar");
        assert_eq!(to_pascal_case("foo_bar_baz"), "FooBarBaz");
        assert_eq!(to_pascal_case("FOO"), "FOO");
        assert_eq!(to_pascal_case(""), "");
    }
}
