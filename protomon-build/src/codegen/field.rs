//! Field code generation.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::context::{to_rust_field_name, GenerationContext};
use crate::descriptor::FieldDescriptorProto;
use crate::Error;

use super::types::{build_full_type, proto_type_to_rust};

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
