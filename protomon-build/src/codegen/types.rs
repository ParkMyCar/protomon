//! Type mapping from protobuf types to Rust types.

use proc_macro2::TokenStream;
use quote::quote;

use crate::context::GenerationContext;
use crate::descriptor::{FieldOptions, Label, Type};
use crate::Error;

/// Rust type information for a proto field.
pub struct RustType {
    /// The base Rust type (without `Option`/`Repeated`/`Box` wrapper).
    pub base_type: TokenStream,
    /// Whether to wrap in `Option<T>`.
    pub is_optional: bool,
    /// Whether this is a repeated field.
    pub is_repeated: bool,
    /// Whether to use `Repeated<T>` vs `Vec<T>`.
    pub use_lazy_repeated: bool,
    /// Whether to wrap in `Box<T>`.
    pub is_boxed: bool,
}

/// Map proto type to Rust type.
///
/// The `auto_box` parameter indicates this field was detected as part of a
/// recursive type cycle and should be automatically boxed.
#[allow(clippy::too_many_arguments)]
pub fn proto_type_to_rust(
    ctx: &GenerationContext,
    proto_type: Type,
    type_name: Option<&str>,
    label: Label,
    is_proto3: bool,
    proto3_optional: bool,
    field_options: Option<&FieldOptions>,
    auto_box: bool,
) -> Result<RustType, Error> {
    let is_repeated = label == Label::Repeated;

    // Determine if field should be optional
    let is_optional = match (is_proto3, label, proto3_optional) {
        // proto3 with explicit `optional` keyword -> `Option<T>`
        (true, Label::Optional, true) => true,
        // proto3: message fields are always optional (absence is meaningful)
        (true, Label::Optional, false) if proto_type == Type::Message => true,
        // proto3: scalar fields use implicit presence (no Option, just default values)
        (true, Label::Optional, false) => false,
        // proto3 doesn't have required, but handle it anyway
        (true, Label::Required, _) => false,
        // proto2: LABEL_OPTIONAL means `Option<T>`
        (false, Label::Optional, _) => true,
        // proto2: LABEL_REQUIRED means no Option
        (false, Label::Required, _) => false,
        // repeated fields are not wrapped in Option
        (_, Label::Repeated, _) => false,
    };

    // Check protomon extensions for vec, boxed, lazy, and fixed_array options
    let use_vec = field_options.map(|o| o.vec).unwrap_or(false);
    let explicit_boxed = field_options.map(|o| o.boxed).unwrap_or(false);
    let is_lazy = field_options.map(|o| o.lazy).unwrap_or(false);
    let fixed_array = field_options.map(|o| o.fixed_array).unwrap_or(0);

    // Box if explicitly requested OR if auto-detected as recursive
    let is_boxed = explicit_boxed || auto_box;

    // Validate that vec option is only used on repeated fields or bytes fields
    if use_vec && !is_repeated && proto_type != Type::Bytes {
        return Err(Error::InvalidOption(
            "[(protomon.vec) = true] can only be used on repeated fields or bytes fields".into(),
        ));
    }

    // Validate that lazy option is only used on message-type fields
    if is_lazy && proto_type != Type::Message {
        return Err(Error::InvalidOption(
            "[(protomon.lazy) = true] can only be used on message-type fields".into(),
        ));
    }

    // Validate that fixed_array option is only used on bytes fields
    if fixed_array > 0 && proto_type != Type::Bytes {
        return Err(Error::InvalidOption(
            "[(protomon.fixed_array) = N] can only be used on bytes fields".into(),
        ));
    }

    // Validate that fixed_array size is at most 32 (Rust's Default trait limit)
    if fixed_array > 32 {
        return Err(Error::InvalidOption(format!(
            "[(protomon.fixed_array) = {}] exceeds maximum size of 32. \
                 Rust's Default trait is only implemented for arrays up to [T; 32].",
            fixed_array
        )));
    }

    let base_type =
        scalar_type_to_rust_inner(ctx, proto_type, type_name, is_lazy, fixed_array, use_vec)?;

    Ok(RustType {
        base_type,
        is_optional,
        is_repeated,
        use_lazy_repeated: !use_vec,
        is_boxed,
    })
}

/// Map proto type to Rust type for map keys.
///
/// Map keys can only be integral types or strings (not floats, bytes, or messages).
pub fn map_key_type_to_rust(proto_type: Type) -> Result<TokenStream, Error> {
    let tokens = match proto_type {
        Type::Int32 => quote!(i32),
        Type::Int64 => quote!(i64),
        Type::Uint32 => quote!(u32),
        Type::Uint64 => quote!(u64),
        Type::Sint32 => quote!(protomon::codec::Sint32),
        Type::Sint64 => quote!(protomon::codec::Sint64),
        Type::Fixed32 => quote!(protomon::codec::Fixed32),
        Type::Fixed64 => quote!(protomon::codec::Fixed64),
        Type::Sfixed32 => quote!(protomon::codec::Sfixed32),
        Type::Sfixed64 => quote!(protomon::codec::Sfixed64),
        Type::Bool => quote!(bool),
        Type::String => quote!(String),
        _ => {
            return Err(Error::InvalidOption(format!(
                "Invalid map key type {:?}. Map keys must be integral types, bool, or string.",
                proto_type
            )));
        }
    };
    Ok(tokens)
}

/// Map proto scalar/message type to base Rust type (public version for map values).
pub fn scalar_type_to_rust(
    ctx: &GenerationContext,
    proto_type: Type,
    type_name: Option<&str>,
) -> Result<TokenStream, Error> {
    scalar_type_to_rust_inner(ctx, proto_type, type_name, false, 0, false)
}

/// Map proto scalar/message type to base Rust type.
///
/// For message types, `is_lazy` controls whether to wrap in `LazyMessage<T>`.
/// For bytes types, `fixed_array` > 0 uses `[u8; N]` instead of `ProtoBytes`,
/// and `use_vec` uses `Vec<u8>` instead of `ProtoBytes`.
fn scalar_type_to_rust_inner(
    ctx: &GenerationContext,
    proto_type: Type,
    type_name: Option<&str>,
    is_lazy: bool,
    fixed_array: u32,
    use_vec: bool,
) -> Result<TokenStream, Error> {
    let tokens = match proto_type {
        // Integers - standard encoding
        Type::Int32 => quote!(i32),
        Type::Int64 => quote!(i64),
        Type::Uint32 => quote!(u32),
        Type::Uint64 => quote!(u64),

        // Integers - zigzag encoding
        Type::Sint32 => quote!(protomon::codec::Sint32),
        Type::Sint64 => quote!(protomon::codec::Sint64),

        // Integers - fixed encoding
        Type::Fixed32 => quote!(protomon::codec::Fixed32),
        Type::Fixed64 => quote!(protomon::codec::Fixed64),
        Type::Sfixed32 => quote!(protomon::codec::Sfixed32),
        Type::Sfixed64 => quote!(protomon::codec::Sfixed64),

        // Floating point
        Type::Float => quote!(f32),
        Type::Double => quote!(f64),

        // Bool
        Type::Bool => quote!(bool),

        // String and bytes
        Type::String => quote!(protomon::codec::ProtoString),
        Type::Bytes => {
            if fixed_array > 0 {
                let size = fixed_array as usize;
                quote!([u8; #size])
            } else if use_vec {
                quote!(Vec<u8>)
            } else {
                quote!(protomon::codec::ProtoBytes)
            }
        }

        // Message type
        Type::Message => {
            let type_name = type_name
                .ok_or_else(|| Error::DecodeError("Message type must have type_name".into()))?;
            let path: syn::Path = if let Some(extern_path) = ctx.config.extern_paths.get(type_name)
            {
                syn::parse_str(extern_path).map_err(|e| {
                    Error::SynParse(format!("Invalid extern_path '{}': {}", extern_path, e))
                })?
            } else {
                let rust_type = ctx.resolve_type(type_name).unwrap_or_else(|| {
                    // Fallback: just use the last component of the type name
                    type_name
                        .rsplit('.')
                        .next()
                        .unwrap_or(type_name)
                        .to_string()
                });
                syn::parse_str(&rust_type).map_err(|e| {
                    Error::SynParse(format!("Invalid type path '{}': {}", rust_type, e))
                })?
            };
            // Wrap in LazyMessage only if lazy option is set
            if is_lazy {
                quote!(protomon::codec::LazyMessage<#path>)
            } else {
                quote!(#path)
            }
        }

        // Enum type - represented as i32
        Type::Enum => quote!(i32),

        // Group (deprecated)
        Type::Group => {
            return Err(Error::DecodeError("Group types are not supported".into()));
        }
    };
    Ok(tokens)
}

/// Build the full Rust type including `Option`/`Repeated`/`Box` wrappers.
///
/// The wrapping order is:
/// - For optional boxed fields: `Option<Box<T>>` (`None` doesn't allocate)
/// - For repeated boxed fields: `Repeated<Box<T>>` or `Vec<Box<T>>` (each element is boxed)
/// - For singular boxed fields: `Box<T>`
pub fn build_full_type(rust_type: &RustType) -> TokenStream {
    let base = &rust_type.base_type;

    // First, wrap in Box if requested (Box wraps the base type, not the collection)
    let inner = if rust_type.is_boxed {
        quote!(Box<#base>)
    } else {
        quote!(#base)
    };

    // Then apply repeated/optional wrappers
    if rust_type.is_repeated {
        if rust_type.use_lazy_repeated {
            quote!(protomon::codec::Repeated<#inner>)
        } else {
            quote!(Vec<#inner>)
        }
    } else if rust_type.is_optional {
        quote!(Option<#inner>)
    } else {
        inner
    }
}
