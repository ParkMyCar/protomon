//! Derive macros for protomon.
//!
//! Provides `#[derive(ProtoMessage)]` for generating protobuf encoding/decoding.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Field, Ident, Result, Type};

/// Derive macro for implementing `ProtoMessage` trait.
///
/// # Example
///
/// ```ignore
/// #[derive(ProtoMessage)]
/// pub struct Person {
///     #[proto(tag = 1)]
///     name: ProtoString,
///     #[proto(tag = 2)]
///     id: i32,
///     #[proto(tag = 3, optional)]
///     email: Option<ProtoString>,
///     #[proto(tag = 4, repeated)]
///     phones: Repeated<LazyMessage<PhoneNumber>>,
/// }
/// ```
///
/// The wire type for each field is inferred from the Rust type using
/// `<T as ProtoType>::WIRE_TYPE`, so there's no need to specify it manually.
#[proc_macro_derive(ProtoMessage, attributes(proto))]
pub fn derive_proto_message(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

    match impl_proto_message(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

struct FieldInfo<'a> {
    name: &'a Ident,
    ty: &'a Type,
    tag: u32,
    repeated: bool,
    optional: bool,
}

/// Extract the inner type from a generic type like `Vec<T>` or `Repeated<T>`.
fn get_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                    return Some(inner);
                }
            }
        }
    }
    None
}

/// Check if a type is `Vec<...>`.
fn is_vec_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Vec";
        }
    }
    false
}

fn impl_proto_message(input: &DeriveInput) -> Result<TokenStream2> {
    let name = &input.ident;

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => return Err(syn::Error::new_spanned(input, "only named fields supported")),
        },
        _ => return Err(syn::Error::new_spanned(input, "only structs supported")),
    };

    let field_info: Vec<FieldInfo> = fields
        .iter()
        .map(|f| {
            let (tag, repeated, optional) = parse_proto_attrs(f)?;
            Ok(FieldInfo {
                name: f.ident.as_ref().unwrap(),
                ty: &f.ty,
                tag,
                repeated,
                optional,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let decode_impl = generate_decode(name, &field_info);
    let encode_impl = generate_encode(&field_info);
    let len_impl = generate_len(&field_info);

    Ok(quote! {
        impl protomon::codec::ProtoMessage for #name {
            #decode_impl
            #encode_impl
            #len_impl
        }
    })
}

/// Parse #[proto(tag = N, repeated, optional)] attributes.
fn parse_proto_attrs(field: &Field) -> Result<(u32, bool, bool)> {
    let mut tag = None;
    let mut repeated = false;
    let mut optional = false;

    for attr in &field.attrs {
        if attr.path().is_ident("proto") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("tag") {
                    let value: syn::LitInt = meta.value()?.parse()?;
                    tag = Some(value.base10_parse::<u32>()?);
                } else if meta.path.is_ident("repeated") {
                    repeated = true;
                } else if meta.path.is_ident("optional") {
                    optional = true;
                }
                Ok(())
            })?;
        }
    }

    match tag {
        Some(t) => Ok((t, repeated, optional)),
        None => Err(syn::Error::new_spanned(
            field,
            "missing #[proto(tag = N)] attribute",
        )),
    }
}


fn generate_decode(name: &Ident, fields: &[FieldInfo]) -> TokenStream2 {
    // Generate field initializations using ProtoDecode::init
    let field_inits = fields.iter().map(|f| {
        let fname = f.name;
        let fty = f.ty;
        let tag = f.tag;

        quote! {
            let mut #fname: #fty = <#fty as protomon::codec::ProtoDecode>::init(buf.clone(), #tag);
        }
    });

    // Generate match arms for decoding - all fields use decode_into uniformly
    let decode_arms = fields.iter().map(|f| {
        let fname = f.name;
        let fty = f.ty;
        let tag = f.tag;

        quote! {
            #tag => <#fty as protomon::codec::ProtoDecode>::decode_into(&mut slice, &mut #fname, value_offset)?,
        }
    });

    let field_names = fields.iter().map(|f| f.name);

    quote! {
        fn decode_message(buf: bytes::Bytes) -> Result<Self, protomon::error::DecodeErrorKind> {
            use bytes::Buf;
            use protomon::codec::ProtoDecode;
            use protomon::wire::{decode_key, skip_field};

            let mut slice = &buf[..];
            #(#field_inits)*

            while slice.has_remaining() {
                let (wire_type, tag) = decode_key(&mut slice)?;
                let value_offset = buf.len() - slice.len();
                match tag {
                    #(#decode_arms)*
                    _ => skip_field(wire_type, &mut slice)?,
                }
            }

            Ok(#name { #(#field_names),* })
        }
    }
}

fn generate_encode(fields: &[FieldInfo]) -> TokenStream2 {
    let encode_fields = fields.iter().map(|f| {
        let fname = f.name;
        let fty = f.ty;
        let tag = f.tag;

        if f.repeated {
            let inner_ty = get_inner_type(fty);
            // Vec<T> iteration yields &T, Repeated<T> iteration yields Result<T, _>
            if is_vec_type(fty) {
                // Vec<T>: for value in &self.vec yields &T
                quote! {
                    for value in &self.#fname {
                        protomon::wire::encode_key(<#inner_ty as protomon::codec::ProtoType>::WIRE_TYPE, #tag, buf);
                        protomon::codec::ProtoEncode::encode(value, buf);
                    }
                }
            } else {
                // Repeated<T>: for result in &self.repeated yields Result<T, _>
                // Skip errors during encoding
                quote! {
                    for result in &self.#fname {
                        if let Ok(value) = result {
                            protomon::wire::encode_key(<#inner_ty as protomon::codec::ProtoType>::WIRE_TYPE, #tag, buf);
                            protomon::codec::ProtoEncode::encode(&value, buf);
                        }
                    }
                }
            }
        } else if f.optional {
            // Optional fields only encode if Some
            quote! {
                if let Some(ref value) = self.#fname {
                    protomon::wire::encode_key(<#fty as protomon::codec::ProtoType>::WIRE_TYPE, #tag, buf);
                    protomon::codec::ProtoEncode::encode(value, buf);
                }
            }
        } else {
            // Regular fields only encode if not default (proto3 semantics)
            quote! {
                if self.#fname != <#fty as Default>::default() {
                    protomon::wire::encode_key(<#fty as protomon::codec::ProtoType>::WIRE_TYPE, #tag, buf);
                    <#fty as protomon::codec::ProtoEncode>::encode(&self.#fname, buf);
                }
            }
        }
    });

    quote! {
        fn encode_message<B: bytes::BufMut>(&self, buf: &mut B) {
            #(#encode_fields)*
        }
    }
}

fn generate_len(fields: &[FieldInfo]) -> TokenStream2 {
    let len_fields = fields.iter().map(|f| {
        let fname = f.name;
        let fty = f.ty;
        let tag = f.tag;

        if f.repeated {
            // Vec<T> iteration yields &T, Repeated<T> iteration yields Result<T, _>
            if is_vec_type(fty) {
                // Vec<T>: for value in &self.vec yields &T
                quote! {
                    let key_len = protomon::wire::encoded_key_len(#tag);
                    for value in &self.#fname {
                        len += key_len + protomon::codec::ProtoEncode::encoded_len(value);
                    }
                }
            } else {
                // Repeated<T>: for result in &self.repeated yields Result<T, _>
                // Skip errors during length calculation
                quote! {
                    let key_len = protomon::wire::encoded_key_len(#tag);
                    for result in &self.#fname {
                        if let Ok(value) = result {
                            len += key_len + protomon::codec::ProtoEncode::encoded_len(&value);
                        }
                    }
                }
            }
        } else if f.optional {
            // Optional fields only count if Some
            quote! {
                if let Some(ref value) = self.#fname {
                    len += protomon::wire::encoded_key_len(#tag) + protomon::codec::ProtoEncode::encoded_len(value);
                }
            }
        } else {
            // Regular fields only count if not default (proto3 semantics)
            quote! {
                if self.#fname != <#fty as Default>::default() {
                    len += protomon::wire::encoded_key_len(#tag) + <#fty as protomon::codec::ProtoEncode>::encoded_len(&self.#fname);
                }
            }
        }
    });

    quote! {
        fn encoded_message_len(&self) -> usize {
            let mut len = 0usize;
            #(#len_fields)*
            len
        }
    }
}
