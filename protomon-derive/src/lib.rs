//! Derive macros for protomon.
//!
//! Provides `#[derive(ProtoMessage)]` and `#[derive(ProtoOneof)]` for generating
//! protobuf encoding/decoding implementations.

use std::ops::RangeInclusive;

use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{DeriveInput, Field, Fields, Ident, Result, Type, Variant};

/// Minimum value of a protobuf tag.
const MINIMUM_TAG_VAL: u32 = 1;
/// Maximum value of a protobuf tag.
const MAXIMUM_TAG_VAL: u32 = (1 << 29) - 1;
/// Range of tag values that is reserved by Google.
const RESERVED_TAG_RANGE: RangeInclusive<u32> = 19000..=19999;

/// Derive macro for implementing `ProtoMessage` trait.
///
/// Note: You must also derive or implement `Default` for your struct.
///
/// # Example
///
/// ```ignore
/// #[derive(Default, ProtoMessage)]
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
#[proc_macro_derive(ProtoMessage, attributes(proto))]
pub fn derive_proto_message(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

    match impl_proto_message(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Metadata for a single field from a `struct`.
struct FieldMetadata<'a> {
    /// Name of the field.
    name: &'a Ident,
    /// Type of the field.
    ty: &'a Type,
    /// Attributes from the `#[proto(...)]` annotation.
    attrs: FieldAttrs,
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

    let field_info: Vec<FieldMetadata> = fields
        .iter()
        .map(parse_field_metadata)
        .collect::<Result<Vec<_>>>()?;

    // Check for duplicate tags
    let mut seen_tags = std::collections::BTreeSet::new();
    for f in &field_info {
        for tag in f.attrs.all_tags() {
            // insert() returns false if the value already existed
            if !seen_tags.insert(*tag) {
                let msg = format!("duplicate tag '{tag}' (tags must be unique across all fields)");
                return Err(syn::Error::new_spanned(f.name, msg));
            }
        }
    }

    let decode_into_impl = generate_decode_into(&field_info);
    let encode_impl = generate_encode(&field_info);
    let len_impl = generate_len(&field_info);

    Ok(quote! {
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
            ) -> Result<(), protomon::error::DecodeError> {
                use bytes::Buf;
                let len = protomon::wire::decode_len(buf)?;
                if buf.remaining() < len {
                    return Err(protomon::error::DecodeError::unexpected_end_of_buffer());
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

struct FieldAttrs {
    /// The protobuf tag number.
    tag: u32,
    /// Whether this is a repeated field.
    repeated: bool,
    /// Whether this is an optional field (Option<T>).
    optional: bool,
    /// Whether this is a map field.
    map: bool,
    /// If this is a oneof field, contains all tags that belong to this oneof.
    oneof_tags: Option<Vec<u32>>,
    /// If this is a required (non-nullable) oneof field.
    oneof_required: bool,
    /// Whether this field stores unknown fields.
    unknown: bool,
}

impl FieldAttrs {
    /// Returns all of the tag values this field is annotated with.
    pub fn all_tags(&self) -> impl Iterator<Item = &u32> {
        let single_tag = if self.oneof_tags.is_some() || self.unknown {
            None
        } else {
            Some(&self.tag)
        };
        single_tag
            .into_iter()
            .chain(self.oneof_tags.iter().flatten())
    }
}

/// Raw attributes parsed from `#[proto(...)]` on a field.
#[derive(Debug, Default, FromMeta)]
#[darling(default)]
struct RawProtoFieldAttrs {
    tag: Option<u32>,
    repeated: bool,
    optional: bool,
    map: bool,
    oneof: bool,
    tags: Option<String>,
    required: bool,
    unknown: bool,
}

/// Validates that a tag number is within the valid Protocol Buffers range.
fn validate_tag(tag: u32, span: proc_macro2::Span) -> Result<()> {
    if !(MINIMUM_TAG_VAL..=MAXIMUM_TAG_VAL).contains(&tag) || RESERVED_TAG_RANGE.contains(&tag) {
        let msg = format!(
            "Tag number '{}' is invalid. Valid tag numbers are in the range [{}, {}], excluding [{}, {}]",
            tag,
            MINIMUM_TAG_VAL,
            MAXIMUM_TAG_VAL,
            RESERVED_TAG_RANGE.start(),
            RESERVED_TAG_RANGE.end(),
        );
        return Err(syn::Error::new(span, msg));
    }

    Ok(())
}

/// Parse `#[proto(...)]` attributes from a field and return complete metadata.
fn parse_field_metadata(field: &Field) -> Result<FieldMetadata<'_>> {
    // Find the #[proto(...)] attribute and parse it with darling
    let raw = field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("proto"))
        .map(|attr| RawProtoFieldAttrs::from_meta(&attr.meta))
        .transpose()
        .map_err(|e| syn::Error::new_spanned(field, e.to_string()))?
        .unwrap_or_default();

    // Parse oneof tags string into Vec<u32>
    let oneof_tags = if raw.oneof {
        let Some(tags_str) = raw.tags else {
            let msg = "oneof field requires tags = \"1, 2, 3\" attribute";
            return Err(syn::Error::new_spanned(field, msg));
        };
        let tags = tags_str
            .split(',')
            .map(|s| {
                let parsed_tag = s
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| syn::Error::new_spanned(field, "invalid tag in tags list"))?;
                validate_tag(parsed_tag, field.span())?;
                Ok(parsed_tag)
            })
            .collect::<Result<Vec<u32>>>()?;

        Some(tags)
    } else {
        None
    };

    if raw.repeated && raw.optional {
        return Err(syn::Error::new_spanned(
            field,
            "field cannot be both 'repeated' and 'optional'",
        ));
    }

    if raw.map && raw.repeated {
        return Err(syn::Error::new_spanned(
            field,
            "map fields cannot also be 'repeated' (maps are implicitly repeated)",
        ));
    }

    if raw.map && raw.optional {
        return Err(syn::Error::new_spanned(
            field,
            "map fields cannot be 'optional'",
        ));
    }

    if raw.map && raw.oneof {
        return Err(syn::Error::new_spanned(
            field,
            "map fields cannot be part of a oneof",
        ));
    }

    if raw.oneof && raw.repeated {
        return Err(syn::Error::new_spanned(
            field,
            "oneof fields cannot be 'repeated'",
        ));
    }

    if raw.required && !raw.oneof {
        return Err(syn::Error::new_spanned(
            field,
            "'required' attribute is only valid for oneof fields",
        ));
    }

    // For oneof/unknown fields, tag is not required (use 0 as placeholder)
    let tag = if raw.oneof || raw.unknown {
        if let Some(t) = raw.tag {
            validate_tag(t, field.span())?;
            t
        } else {
            0
        }
    } else {
        match raw.tag {
            Some(t) => {
                validate_tag(t, field.span())?;
                t
            }
            None => {
                return Err(syn::Error::new_spanned(
                    field,
                    "missing #[proto(tag = N)] attribute",
                ))
            }
        }
    };

    Ok(FieldMetadata {
        name: field.ident.as_ref().unwrap(),
        ty: &field.ty,
        attrs: FieldAttrs {
            tag,
            repeated: raw.repeated,
            optional: raw.optional,
            map: raw.map,
            oneof_tags,
            oneof_required: raw.oneof && raw.required,
            unknown: raw.unknown,
        },
    })
}

fn generate_decode_into(fields: &[FieldMetadata]) -> TokenStream2 {
    // Find the unknown field if present
    let unknown_field = fields.iter().find(|f| f.attrs.unknown);
    let has_unknown_field = unknown_field.is_some();

    // Filter out the unknown field from normal processing
    let regular_fields: Vec<_> = fields.iter().filter(|f| !f.attrs.unknown).collect();

    // Generate field initializations that work directly on dst
    // Only repeated fields need init_repeated - other fields are already default
    // (caller is responsible for providing a default-initialized dst)
    let field_inits = regular_fields.iter().filter_map(|f| {
        if f.attrs.repeated {
            let fname = f.name;
            let tag = f.attrs.tag;
            Some(quote! {
                protomon::codec::ProtoRepeated::init_repeated(&mut dst.#fname, &buf, #tag);
            })
        } else {
            None
        }
    });

    // Collect oneof fields, separating required from optional
    let oneof_fields: Vec<&&FieldMetadata> = regular_fields
        .iter()
        .filter(|f| f.attrs.oneof_tags.is_some())
        .collect();
    let (required_oneof_fields, optional_oneof_fields): (
        Vec<&&FieldMetadata>,
        Vec<&&FieldMetadata>,
    ) = oneof_fields
        .into_iter()
        .partition(|f| f.attrs.oneof_required);

    // Generate temporary variables for required oneofs
    let required_oneof_temps = required_oneof_fields.iter().map(|f| {
        let fname = f.name;
        let temp_name = format_ident!("__oneof_{}", fname);
        let fty = f.ty; // This is already the non-Option type for required oneofs
        quote! {
            let mut #temp_name: Option<#fty> = None;
        }
    });

    // If we have an unknown field, initialize a buffer to collect unknown field bytes
    let unknown_buffer_init = if has_unknown_field {
        quote! {
            use alloc::vec::Vec;
            let mut unknown_buf = Vec::new();
        }
    } else {
        quote!()
    };

    // Generate match arms for regular fields (excluding unknown field)
    let regular_decode_arms = regular_fields.iter().filter_map(|f| {
        // Skip oneof fields - they're handled separately
        if f.attrs.oneof_tags.is_some() {
            return None;
        }

        let fname = f.name;
        let fty = f.ty;
        let tag = f.attrs.tag;

        if f.attrs.map {
            // Map fields decode a single entry per tag occurrence
            Some(quote! {
                #tag => protomon::codec::ProtoMap::decode_entry(&mut dst.#fname, &mut buf)?,
            })
        } else if f.attrs.repeated {
            // For Vec<T> repeated fields, use decode_repeated_into which handles packed encoding
            // Extract the inner type T from Vec<T>
            if let Some(inner_ty) = extract_vec_inner_type(fty) {
                Some(quote! {
                    #tag => protomon::codec::decode_repeated_into::<#inner_ty, _>(wire_type, &mut buf, &mut dst.#fname, value_offset)?,
                })
            } else {
                // For Repeated<T> or other types, use the standard decode_into
                Some(quote! {
                    #tag => <#fty as protomon::codec::ProtoDecode>::decode_into(&mut buf, &mut dst.#fname, value_offset)?,
                })
            }
        } else {
            Some(quote! {
                #tag => <#fty as protomon::codec::ProtoDecode>::decode_into(&mut buf, &mut dst.#fname, value_offset)?,
            })
        }
    });

    // Generate match arms for optional oneof fields (field type is Option<T>)
    let optional_oneof_decode_arms = optional_oneof_fields.iter().flat_map(|f| {
        let fname = f.name;
        let fty = f.ty;
        let tags = f.attrs.oneof_tags.as_ref().unwrap();

        // Extract the inner type from Option<T> for the decode_oneof_field call
        let inner_ty = extract_option_inner_type(fty);

        tags.iter().map(move |tag| {
            match inner_ty {
                Some(inner) => quote! {
                    #tag => {
                        protomon::codec::decode_oneof_field::<#inner, _>(&mut dst.#fname, tag, wire_type, &mut buf, value_offset)?;
                    }
                },
                None => quote! {
                    #tag => {
                        compile_error!(concat!("nullable oneof field `", stringify!(#fname), "` must have type Option<T>"));
                    }
                },
            }
        })
    });

    // Generate match arms for required oneof fields (decode into temp Option<T>)
    let required_oneof_decode_arms = required_oneof_fields.iter().flat_map(|f| {
        let fname = f.name;
        let temp_name = format_ident!("__oneof_{}", fname);
        let fty = f.ty; // Already the non-Option type
        let tags = f.attrs.oneof_tags.as_ref().unwrap();

        tags.iter().map(move |tag| {
            quote! {
                #tag => {
                    protomon::codec::decode_oneof_field::<#fty, _>(&mut #temp_name, tag, wire_type, &mut buf, value_offset)?;
                }
            }
        })
    });

    // Generate validation and assignment for required oneofs after decode loop
    let required_oneof_validations = required_oneof_fields.iter().map(|f| {
        let fname = f.name;
        let temp_name = format_ident!("__oneof_{}", fname);
        // Use the first tag in the oneof as the identifying tag for errors
        let first_tag = f.attrs.oneof_tags.as_ref().unwrap()[0];

        quote! {
            dst.#fname = #temp_name.ok_or_else(|| protomon::error::DecodeError::missing_required_oneof(#first_tag))?;
        }
    });

    // Generate the default match arm (for unknown fields)
    let default_arm = if has_unknown_field {
        quote! {
            _ => {
                // Collect unknown field: we need to preserve the key and the value
                // First, encode the key (tag and wire type) into unknown_buf
                use protomon::leb128::LebCodec;
                let key = (tag << 3) | wire_type as u32;
                key.encode_leb128(&mut unknown_buf);

                // Then copy the field value into unknown_buf
                match wire_type {
                    protomon::wire::WireType::Varint => {
                        // Read the varint and encode it to unknown_buf
                        let (val, _) = u64::decode_leb128_buf(&mut buf)?;
                        val.encode_leb128(&mut unknown_buf);
                    }
                    protomon::wire::WireType::I64 => {
                        // Copy 8 bytes directly without intermediate Bytes allocation
                        if buf.remaining() < 8 {
                            return Err(protomon::error::DecodeError::unexpected_end_of_buffer());
                        }
                        unknown_buf.extend_from_slice(&buf.chunk()[..8]);
                        buf.advance(8);
                    }
                    protomon::wire::WireType::Len => {
                        // Read length and copy length + data
                        let len = protomon::wire::decode_len(&mut buf)?;
                        // Encode the length to unknown_buf
                        (len as u64).encode_leb128(&mut unknown_buf);
                        // Copy the data directly without intermediate Bytes allocation
                        if buf.remaining() < len {
                            return Err(protomon::error::DecodeError::unexpected_end_of_buffer());
                        }
                        unknown_buf.extend_from_slice(&buf.chunk()[..len]);
                        buf.advance(len);
                    }
                    protomon::wire::WireType::I32 => {
                        // Copy 4 bytes directly without intermediate Bytes allocation
                        if buf.remaining() < 4 {
                            return Err(protomon::error::DecodeError::unexpected_end_of_buffer());
                        }
                        unknown_buf.extend_from_slice(&buf.chunk()[..4]);
                        buf.advance(4);
                    }
                    protomon::wire::WireType::SGroup | protomon::wire::WireType::EGroup => {
                        return Err(protomon::error::DecodeError::deprecated_group_encoding());
                    }
                }
            }
        }
    } else {
        quote! {
            _ => skip_field(wire_type, &mut buf)?,
        }
    };

    // After the decode loop, assign the unknown bytes to the unknown field
    let unknown_field_assignment = if let Some(unk_field) = unknown_field {
        let fname = unk_field.name;
        quote! {
            dst.#fname = bytes::Bytes::from(unknown_buf);
        }
    } else {
        quote!()
    };

    quote! {
        #[inline(always)]
        fn decode_message_into(buf: bytes::Bytes, dst: &mut Self) -> Result<(), protomon::error::DecodeError> {
            use bytes::Buf;
            use protomon::codec::ProtoDecode;
            use protomon::wire::{decode_key, skip_field};

            let original_len = buf.len();
            let mut buf = buf;
            #(#field_inits)*
            #(#required_oneof_temps)*
            #unknown_buffer_init

            while buf.has_remaining() {
                let (wire_type, tag) = decode_key(&mut buf)?.into_parts();
                let value_offset = original_len - buf.remaining();
                match tag {
                    #(#regular_decode_arms)*
                    #(#optional_oneof_decode_arms)*
                    #(#required_oneof_decode_arms)*
                    #default_arm
                }
            }

            #(#required_oneof_validations)*
            #unknown_field_assignment

            Ok(())
        }
    }
}

fn generate_encode(fields: &[FieldMetadata]) -> TokenStream2 {
    // Find the unknown field if present
    let unknown_field = fields.iter().find(|f| f.attrs.unknown);

    // Filter out the unknown field from normal processing
    let regular_fields: Vec<_> = fields.iter().filter(|f| !f.attrs.unknown).collect();

    let encode_fields = regular_fields.iter().map(|f| {
        let fname = f.name;
        let fty = f.ty;
        let tag = f.attrs.tag;

        if f.attrs.oneof_tags.is_some() && f.attrs.oneof_required {
            // Required oneof fields encode directly (field type is T, not Option<T>)
            quote! {
                self.#fname.encode_variant(buf);
            }
        } else if f.attrs.oneof_tags.is_some() {
            // Optional oneof fields use the specialized encode helper
            quote! {
                protomon::codec::encode_oneof_field(&self.#fname, buf);
            }
        } else if f.attrs.map {
            // Map fields encode all entries with their field keys
            quote! {
                protomon::codec::ProtoMap::encode_map(&self.#fname, #tag, buf);
            }
        } else if f.attrs.repeated {
            // Both Vec<T> and Repeated<T> implement ProtoRepeated
            quote! {
                protomon::codec::ProtoRepeated::encode_repeated(&self.#fname, #tag, buf);
            }
        } else if f.attrs.optional {
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

    // Append the unknown field bytes at the end
    let encode_unknown = if let Some(unk_field) = unknown_field {
        let fname = unk_field.name;
        quote! {
            // Append unknown fields
            if !self.#fname.is_empty() {
                use bytes::Buf;
                buf.put_slice(&self.#fname);
            }
        }
    } else {
        quote!()
    };

    quote! {
        fn encode_message<B: bytes::BufMut>(&self, buf: &mut B) {
            #(#encode_fields)*
            #encode_unknown
        }
    }
}

fn generate_len(fields: &[FieldMetadata]) -> TokenStream2 {
    // Find the unknown field if present
    let unknown_field = fields.iter().find(|f| f.attrs.unknown);

    // Filter out the unknown field from normal processing
    let regular_fields: Vec<_> = fields.iter().filter(|f| !f.attrs.unknown).collect();

    let len_fields = regular_fields.iter().map(|f| {
        let fname = f.name;
        let fty = f.ty;
        let tag = f.attrs.tag;

        if f.attrs.oneof_tags.is_some() && f.attrs.oneof_required {
            // Required oneof fields use direct length calculation
            quote! {
                len += self.#fname.encoded_variant_len();
            }
        } else if f.attrs.oneof_tags.is_some() {
            // Optional oneof fields use the specialized len helper
            quote! {
                len += protomon::codec::encoded_oneof_field_len(&self.#fname);
            }
        } else if f.attrs.map {
            // Map fields include their own field keys
            quote! {
                len += protomon::codec::ProtoMap::encoded_map_len(&self.#fname, #tag);
            }
        } else if f.attrs.repeated {
            // Both Vec<T> and Repeated<T> implement ProtoRepeated
            quote! {
                len += protomon::codec::ProtoRepeated::encoded_repeated_len(&self.#fname, #tag);
            }
        } else if f.attrs.optional {
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

    // Include the unknown field length
    let len_unknown = if let Some(unk_field) = unknown_field {
        let fname = unk_field.name;
        quote! {
            len += self.#fname.len();
        }
    } else {
        quote!()
    };

    quote! {
        fn encoded_message_len(&self) -> usize {
            let mut len = 0usize;
            #(#len_fields)*
            #len_unknown
            len
        }
    }
}

/// Extract the inner type from an Option<T> type.
/// Returns None if the type is not an Option, which indicates a configuration error.
fn extract_option_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

/// Extract the inner type from a Vec<T> type.
/// Returns None if the type is not a Vec (e.g., it's Repeated<T>).
fn extract_vec_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

/// Derive macro for implementing `ProtoOneof` trait on enums.
///
/// Maps protobuf oneofs to Rust enums. Each variant must have exactly one
/// unnamed field and a `#[proto(tag = N)]` attribute.
///
/// # Example
///
/// ```ignore
/// #[derive(ProtoOneof)]
/// pub enum Widget {
///     #[proto(tag = 1)]
///     Quux(i32),
///     #[proto(tag = 2)]
///     Bar(ProtoString),
///     #[proto(tag = 3)]
///     Nested(Box<SomeMessage>),
/// }
///
/// // In a message:
/// #[derive(ProtoMessage)]
/// pub struct Foo {
///     #[proto(oneof, tags = "1, 2, 3")]
///     widget: Option<Widget>,
/// }
/// ```
#[proc_macro_derive(ProtoOneof, attributes(proto))]
pub fn derive_proto_oneof(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

    match impl_proto_oneof(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

struct OneofVariantInfo<'a> {
    name: &'a Ident,
    ty: &'a Type,
    tag: u32,
}

/// Raw attributes parsed from `#[proto(...)]` on a oneof variant.
///
/// Uses darling for declarative parsing. Validation is done separately.
#[derive(Debug, Default, FromMeta)]
#[darling(default)]
struct RawProtoVariantAttrs {
    /// The protobuf tag number for this variant.
    tag: Option<u32>,
}

fn impl_proto_oneof(input: &DeriveInput) -> Result<TokenStream2> {
    let name = &input.ident;

    let variants = match &input.data {
        syn::Data::Enum(data) => &data.variants,
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "ProtoOneof can only be derived for enums",
            ))
        }
    };

    let variant_info: Vec<OneofVariantInfo> = variants
        .iter()
        .map(parse_oneof_variant)
        .collect::<Result<Vec<_>>>()?;

    let decode_variant_impl = generate_oneof_decode(name, &variant_info);
    let encode_variant_impl = generate_oneof_encode(name, &variant_info);
    let encoded_len_impl = generate_oneof_len(name, &variant_info);
    let variant_tag_impl = generate_oneof_tag(name, &variant_info);
    let variant_wire_type_impl = generate_oneof_wire_type(name, &variant_info);

    Ok(quote! {
        impl protomon::codec::ProtoOneof for #name {
            #decode_variant_impl
            #encode_variant_impl
            #encoded_len_impl
            #variant_tag_impl
            #variant_wire_type_impl
        }
    })
}

fn parse_oneof_variant(variant: &Variant) -> Result<OneofVariantInfo<'_>> {
    // Ensure variant has exactly one unnamed field
    let ty = match &variant.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed.first().unwrap().ty,
        _ => {
            return Err(syn::Error::new_spanned(
                variant,
                "oneof variants must have exactly one unnamed field, e.g., `Foo(i32)`",
            ))
        }
    };

    // Find the #[proto(...)] attribute and parse it with darling
    let raw = variant
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("proto"))
        .map(|attr| RawProtoVariantAttrs::from_meta(&attr.meta))
        .transpose()
        .map_err(|e| syn::Error::new_spanned(variant, e.to_string()))?
        .unwrap_or_default();

    match raw.tag {
        Some(t) => {
            validate_tag(t, variant.span())?;
            Ok(OneofVariantInfo {
                name: &variant.ident,
                ty,
                tag: t,
            })
        }
        None => Err(syn::Error::new_spanned(
            variant,
            "missing #[proto(tag = N)] attribute on oneof variant",
        )),
    }
}

fn generate_oneof_decode(enum_name: &Ident, variants: &[OneofVariantInfo]) -> TokenStream2 {
    let decode_arms = variants.iter().map(|v| {
        let vname = v.name;
        let vty = v.ty;
        let tag = v.tag;

        quote! {
            #tag => {
                if wire_type != <#vty as protomon::codec::ProtoType>::WIRE_TYPE {
                    return Err(protomon::error::DecodeError::invalid_wire_type(wire_type as u8));
                }
                let mut value = <#vty as ::core::default::Default>::default();
                <#vty as protomon::codec::ProtoDecode>::decode_into(buf, &mut value, offset)?;
                Ok(Some(#enum_name::#vname(value)))
            }
        }
    });

    quote! {
        fn decode_variant<B: bytes::Buf>(
            tag: u32,
            wire_type: protomon::wire::WireType,
            buf: &mut B,
            offset: usize,
        ) -> Result<Option<Self>, protomon::error::DecodeError> {
            match tag {
                #(#decode_arms)*
                _ => Ok(None),
            }
        }
    }
}

fn generate_oneof_encode(enum_name: &Ident, variants: &[OneofVariantInfo]) -> TokenStream2 {
    let encode_arms = variants.iter().map(|v| {
        let vname = v.name;
        let vty = v.ty;
        let tag = v.tag;

        quote! {
            #enum_name::#vname(ref value) => {
                protomon::wire::encode_key(<#vty as protomon::codec::ProtoType>::WIRE_TYPE, #tag, buf);
                <#vty as protomon::codec::ProtoEncode>::encode(value, buf);
            }
        }
    });

    quote! {
        fn encode_variant<B: bytes::BufMut>(&self, buf: &mut B) {
            match self {
                #(#encode_arms)*
            }
        }
    }
}

fn generate_oneof_len(enum_name: &Ident, variants: &[OneofVariantInfo]) -> TokenStream2 {
    let len_arms = variants.iter().map(|v| {
        let vname = v.name;
        let vty = v.ty;
        let tag = v.tag;

        quote! {
            #enum_name::#vname(ref value) => {
                protomon::wire::encoded_key_len(#tag) + <#vty as protomon::codec::ProtoEncode>::encoded_len(value)
            }
        }
    });

    quote! {
        fn encoded_variant_len(&self) -> usize {
            match self {
                #(#len_arms)*
            }
        }
    }
}

fn generate_oneof_tag(enum_name: &Ident, variants: &[OneofVariantInfo]) -> TokenStream2 {
    let tag_arms = variants.iter().map(|v| {
        let vname = v.name;
        let tag = v.tag;

        quote! {
            #enum_name::#vname(_) => #tag
        }
    });

    quote! {
        fn variant_tag(&self) -> u32 {
            match self {
                #(#tag_arms),*
            }
        }
    }
}

fn generate_oneof_wire_type(enum_name: &Ident, variants: &[OneofVariantInfo]) -> TokenStream2 {
    let wire_type_arms = variants.iter().map(|v| {
        let vname = v.name;
        let vty = v.ty;

        quote! {
            #enum_name::#vname(_) => <#vty as protomon::codec::ProtoType>::WIRE_TYPE
        }
    });

    quote! {
        fn variant_wire_type(&self) -> protomon::wire::WireType {
            match self {
                #(#wire_type_arms),*
            }
        }
    }
}
