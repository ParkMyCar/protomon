//! Enum code generation.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::descriptor::EnumDescriptorProto;
use crate::Error;

/// Generate Rust enum for a proto enum.
///
/// Note: For now, we represent enums as i32 in message fields.
/// This generates a helper enum with From/Into conversions.
pub fn generate_enum(
    _parent_path: &str,
    enum_type: &EnumDescriptorProto,
) -> Result<TokenStream, Error> {
    let name = enum_type.name.as_ref().ok_or(Error::MissingName)?;
    let enum_name = format_ident!("{}", name);

    // Generate variants
    let variants: Vec<TokenStream> = enum_type
        .value
        .iter()
        .map(|v| -> Result<TokenStream, Error> {
            let variant_name = v.name.as_ref().ok_or(Error::MissingName)?;
            let variant_ident = format_ident!("{}", to_pascal_case(variant_name));
            let number = v.number.ok_or(Error::MissingFieldNumber)?;
            let number_lit = proc_macro2::Literal::i32_unsuffixed(number);
            Ok(quote!(#variant_ident = #number_lit))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Generate From<i32> impl
    let from_i32_arms: Vec<TokenStream> = enum_type
        .value
        .iter()
        .map(|v| -> Result<TokenStream, Error> {
            let variant_name = v.name.as_ref().ok_or(Error::MissingName)?;
            let variant_ident = format_ident!("{}", to_pascal_case(variant_name));
            let number = v.number.ok_or(Error::MissingFieldNumber)?;
            let number_lit = proc_macro2::Literal::i32_unsuffixed(number);
            Ok(quote!(#number_lit => Some(Self::#variant_ident)))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Default variant: prefer value with number 0, otherwise use first value
    let default_value = enum_type
        .value
        .iter()
        .find(|v| v.number == Some(0))
        .or_else(|| enum_type.value.first())
        .ok_or_else(|| Error::DecodeError("Enum must have at least one value".into()))?;
    let default_variant_name = default_value.name.as_ref().ok_or(Error::MissingName)?;
    let default_variant = format_ident!("{}", to_pascal_case(default_variant_name));

    Ok(quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(i32)]
        pub enum #enum_name {
            #(#variants),*
        }

        impl #enum_name {
            /// Convert from i32, returning None for unknown values.
            pub fn from_i32(value: i32) -> Option<Self> {
                match value {
                    #(#from_i32_arms,)*
                    _ => None,
                }
            }
        }

        impl From<#enum_name> for i32 {
            fn from(value: #enum_name) -> Self {
                value as i32
            }
        }

        impl Default for #enum_name {
            fn default() -> Self {
                Self::#default_variant
            }
        }
    })
}

/// Convert SCREAMING_SNAKE_CASE to PascalCase.
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.map(|c| c.to_ascii_lowercase()))
                    .collect(),
            }
        })
        .collect()
}
