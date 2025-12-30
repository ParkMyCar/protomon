//! Types and functions related to parsing the input from our proc-macro.

use core::ops::RangeInclusive;
use darling::FromMeta;
use syn::spanned::Spanned;
use syn::{Field, Ident, Result, Type};

/// Minimum value of a protobuf tag.
const MINIMUM_TAG_VAL: u32 = 1;
/// Maximum value of a protobuf tag.
const MAXIMUM_TAG_VAL: u32 = (1 << 29) - 1;
/// Range of tag values that is reserved by Google.
const RESERVED_TAG_RANGE: RangeInclusive<u32> = 19000..=19999;

/// Metadata for a single field annotated with `#[proto(...)]`.
pub struct FieldMetadata<'a> {
    /// Name of the field.
    pub name: &'a Ident,
    /// Type of the field.
    pub ty: &'a Type,
    /// The kind of field parsed from `#[proto(...)]` attributes.
    pub kind: FieldKind,
}

/// The protobuf kind/type of field within a struct.
pub enum FieldKind {
    /// Normal kind of field, if not present will deserialize to the `Default` value.
    Singular { tag: u32 },
    /// Optional field, if not present will deserialize to `None`.
    Optional { tag: u32 },
    /// Repeated field, if not present will deserialize to an empty set.
    Repeated { tag: u32 },
    /// Map field, essentially a `repeated` field but with (key, value).
    Map { tag: u32 },
    /// Oneof field.
    Oneof {
        /// Tag values that make up this `oneof`.
        tags: Vec<u32>,
        /// Will fail deserialization if a tag from the oneof is not present.
        required: bool,
    },
    /// Field to store the raw bytes of unknown tags. One per struct.
    Unknown,
}

impl FieldKind {
    /// Returns all of the tag values this field is annotated with.
    pub fn all_tags(&self) -> impl Iterator<Item = &u32> {
        let iter: Box<dyn Iterator<Item = &u32>> = match self {
            FieldKind::Singular { tag }
            | FieldKind::Optional { tag }
            | FieldKind::Repeated { tag }
            | FieldKind::Map { tag } => Box::new(std::iter::once(tag)),
            FieldKind::Oneof { tags, .. } => Box::new(tags.iter()),
            FieldKind::Unknown => Box::new(std::iter::empty()),
        };
        iter
    }

    /// Returns the single tag for non-oneof fields.
    pub fn tag(&self) -> Option<u32> {
        match self {
            FieldKind::Singular { tag }
            | FieldKind::Optional { tag }
            | FieldKind::Repeated { tag }
            | FieldKind::Map { tag } => Some(*tag),
            _ => None,
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, FieldKind::Unknown)
    }

    pub fn as_oneof(&self) -> Option<(&[u32], bool)> {
        match self {
            FieldKind::Oneof { tags, required } => Some((tags, *required)),
            _ => None,
        }
    }
}

/// Raw attributes parsed from `#[proto(...)]` on a field.
///
/// We parse these and then transform them into a [`FieldKind`] with [`parse_field_metadata`].
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

/// Parse `#[proto(...)]` attributes from a [`Field`], validates them, and returns
/// a complete [`FieldMetadata`].
pub fn parse_field_metadata(field: &Field) -> Result<FieldMetadata<'_>> {
    // Parse the `#[proto(...)]` attribute.
    let raw = field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("proto"))
        .map(|attr| RawProtoFieldAttrs::from_meta(&attr.meta))
        .transpose()
        .map_err(|e| syn::Error::new_spanned(field, e.to_string()))?
        .unwrap_or_default();

    // Validate 'required' is only used with oneof.
    if raw.required && !raw.oneof {
        return Err(syn::Error::new_spanned(
            field,
            "'required' attribute is only valid for oneof fields",
        ));
    }

    // Determine the field kind.
    let kind = match (raw.unknown, raw.oneof, raw.map, raw.repeated, raw.optional) {
        (true, false, false, false, false) => FieldKind::Unknown,
        (false, true, false, false, false) => {
            let Some(tags_str) = raw.tags else {
                return Err(syn::Error::new_spanned(
                    field,
                    "oneof field requires tags = \"1, 2, 3\" attribute",
                ));
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
            FieldKind::Oneof {
                tags,
                required: raw.required,
            }
        }
        (false, false, map @ true, repeated @ false, optional @ false)
        | (false, false, map @ false, repeated @ true, optional @ false)
        | (false, false, map @ false, repeated @ false, optional @ true)
        | (false, false, map @ false, repeated @ false, optional @ false) => {
            let tag = raw.tag.ok_or_else(|| {
                syn::Error::new_spanned(field, "missing #[proto(tag = N)] attribute")
            })?;
            validate_tag(tag, field.span())?;

            // Only one of the values should be set, or none.
            assert!(map ^ repeated ^ optional ^ (!map && !repeated && !optional));
            if map {
                FieldKind::Map { tag }
            } else if repeated {
                FieldKind::Repeated { tag }
            } else if optional {
                FieldKind::Optional { tag }
            } else {
                FieldKind::Singular { tag }
            }
        }
        // All other combinations are invalid - multiple flags set
        _ => {
            return Err(syn::Error::new_spanned(
                field,
                "conflicting field attributes",
            ));
        }
    };

    Ok(FieldMetadata {
        name: field.ident.as_ref().unwrap(),
        ty: &field.ty,
        kind,
    })
}

/// Validates that a tag number is within the valid Protocol Buffers range.
pub fn validate_tag(tag: u32, span: proc_macro2::Span) -> Result<()> {
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
