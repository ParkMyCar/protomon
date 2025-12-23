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

fn impl_proto_message(input: &DeriveInput) -> Result<TokenStream2> {
    let name = &input.ident;

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "only named fields supported",
                ))
            }
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

    let decode_into_impl = generate_decode_into(&field_info);
    let encode_impl = generate_encode(&field_info);
    let len_impl = generate_len(&field_info);

    // Generate Default impl
    let default_fields = field_info.iter().map(|f| {
        let fname = f.name;
        let fty = f.ty;
        quote! {
            #fname: <#fty as Default>::default()
        }
    });

    Ok(quote! {
        impl Default for #name {
            fn default() -> Self {
                Self {
                    #(#default_fields),*
                }
            }
        }

        impl protomon::codec::ProtoType for #name {
            const WIRE_TYPE: protomon::wire::WireType = protomon::wire::WireType::Len;
        }

        impl protomon::codec::ProtoMessage for #name {
            #decode_into_impl
            #encode_impl
            #len_impl
        }

        impl protomon::codec::ProtoDecode for #name {
            #[inline(always)]
            fn decode_into<B: bytes::Buf>(
                buf: &mut B,
                dst: &mut Self,
                _offset: usize,
            ) -> Result<(), protomon::error::DecodeErrorKind> {
                use bytes::Buf;
                let len = protomon::wire::decode_len(buf)?;
                if buf.remaining() < len {
                    return Err(protomon::error::DecodeErrorKind::UnexpectedEndOfBuffer);
                }
                <Self as protomon::codec::ProtoMessage>::decode_message_into(buf.copy_to_bytes(len), dst)?;
                Ok(())
            }
        }

        impl protomon::codec::ProtoEncode for #name {
            #[inline]
            fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
                use protomon::leb128::LebCodec;
                let len = <Self as protomon::codec::ProtoMessage>::encoded_message_len(self);
                (len as u64).encode_leb128(buf);
                <Self as protomon::codec::ProtoMessage>::encode_message(self, buf);
            }

            #[inline]
            fn encoded_len(&self) -> usize {
                use protomon::leb128::LebCodec;
                let len = <Self as protomon::codec::ProtoMessage>::encoded_message_len(self);
                (len as u64).encoded_leb128_len() + len
            }
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

fn generate_decode_into(fields: &[FieldInfo]) -> TokenStream2 {
    // Generate field initializations that work directly on dst
    // Only repeated fields need init_repeated - other fields are already default
    // (caller is responsible for providing a default-initialized dst)
    let field_inits = fields.iter().filter_map(|f| {
        if f.repeated {
            let fname = f.name;
            let tag = f.tag;
            Some(quote! {
                protomon::codec::ProtoRepeated::init_repeated(&mut dst.#fname, &buf, #tag);
            })
        } else {
            None
        }
    });

    // Generate match arms for decoding - all fields use decode_into uniformly
    let decode_arms = fields.iter().map(|f| {
        let fname = f.name;
        let fty = f.ty;
        let tag = f.tag;

        quote! {
            #tag => <#fty as protomon::codec::ProtoDecode>::decode_into(&mut buf, &mut dst.#fname, value_offset)?,
        }
    });

    quote! {
        #[inline(always)]
        fn decode_message_into(buf: bytes::Bytes, dst: &mut Self) -> Result<(), protomon::error::DecodeErrorKind> {
            use bytes::Buf;
            use protomon::codec::ProtoDecode;
            use protomon::wire::{decode_key, skip_field};

            let original_len = buf.len();
            let mut buf = buf;
            #(#field_inits)*

            while buf.has_remaining() {
                let (wire_type, tag) = decode_key(&mut buf)?;
                let value_offset = original_len - buf.remaining();
                match tag {
                    #(#decode_arms)*
                    _ => skip_field(wire_type, &mut buf)?,
                }
            }

            Ok(())
        }
    }
}

fn generate_encode(fields: &[FieldInfo]) -> TokenStream2 {
    let encode_fields = fields.iter().map(|f| {
        let fname = f.name;
        let fty = f.ty;
        let tag = f.tag;

        if f.repeated {
            // Both Vec<T> and Repeated<T> implement ProtoRepeated
            quote! {
                protomon::codec::ProtoRepeated::encode_repeated(&self.#fname, #tag, buf);
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
                if !<#fty as protomon::codec::IsProtoDefault>::is_proto_default(&self.#fname) {
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
            // Both Vec<T> and Repeated<T> implement ProtoRepeated
            quote! {
                len += protomon::codec::ProtoRepeated::encoded_repeated_len(&self.#fname, #tag);
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
                if !<#fty as protomon::codec::IsProtoDefault>::is_proto_default(&self.#fname) {
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
