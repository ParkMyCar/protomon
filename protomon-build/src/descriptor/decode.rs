//! Decoder for FileDescriptorSet from protobuf binary format.

use super::*;
use crate::Error;
use bytes::Buf;
use protomon::wire::WireType;

/// Maximum size for a single message (64MB).
/// This prevents DoS attacks from malicious input with huge length values.
const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// Maximum bytes for a 64-bit varint (10 bytes).
const MAX_VARINT_BYTES: usize = 10;

// Protomon extension field numbers (50001-50099 reserved for protomon)
// These correspond to the extensions defined in proto/protomon/extensions.proto

/// Extension field number for `vec` option: use `Vec<T>` instead of `Repeated<T>`.
const EXT_FIELD_VEC: u32 = 50001;

/// Extension field number for `boxed` option: wrap field in `Box<T>`.
const EXT_FIELD_BOXED: u32 = 50002;

/// Extension field number for `lazy` option: wrap message in `LazyMessage<T>`.
const EXT_FIELD_LAZY: u32 = 50003;

/// Extension field number for `fixed_array` option: use `[u8; N]` for bytes fields.
const EXT_FIELD_FIXED_ARRAY: u32 = 50004;

/// Extension field number for `nullable` option: make oneof nullable.
const EXT_ONEOF_NULLABLE: u32 = 50000;

/// Decode a FileDescriptorSet from protobuf binary data.
pub fn decode_file_descriptor_set(data: &[u8]) -> Result<FileDescriptorSet, Error> {
    let mut buf = data;
    let mut fds = FileDescriptorSet::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            1 => {
                // file: repeated FileDescriptorProto
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                fds.file.push(decode_file_descriptor_proto(msg_data)?);
            }
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(fds)
}

/// Decode a FileDescriptorProto.
fn decode_file_descriptor_proto(data: &[u8]) -> Result<FileDescriptorProto, Error> {
    let mut buf = data;
    let mut fdp = FileDescriptorProto::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            1 => fdp.name = Some(decode_string(&mut buf)?),
            2 => fdp.package = Some(decode_string(&mut buf)?),
            3 => fdp.dependency.push(decode_string(&mut buf)?),
            4 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                fdp.message_type.push(decode_descriptor_proto(msg_data)?);
            }
            5 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                fdp.enum_type.push(decode_enum_descriptor_proto(msg_data)?);
            }
            12 => fdp.syntax = Some(decode_string(&mut buf)?),
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(fdp)
}

/// Decode a DescriptorProto (message type).
fn decode_descriptor_proto(data: &[u8]) -> Result<DescriptorProto, Error> {
    let mut buf = data;
    let mut dp = DescriptorProto::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            1 => dp.name = Some(decode_string(&mut buf)?),
            2 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                dp.field.push(decode_field_descriptor_proto(msg_data)?);
            }
            3 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                dp.nested_type.push(decode_descriptor_proto(msg_data)?);
            }
            4 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                dp.enum_type.push(decode_enum_descriptor_proto(msg_data)?);
            }
            7 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                dp.options = Some(decode_message_options(msg_data)?);
            }
            8 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                dp.oneof_decl.push(decode_oneof_descriptor_proto(msg_data)?);
            }
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(dp)
}

/// Decode a FieldDescriptorProto.
fn decode_field_descriptor_proto(data: &[u8]) -> Result<FieldDescriptorProto, Error> {
    let mut buf = data;
    let mut fdp = FieldDescriptorProto::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            1 => fdp.name = Some(decode_string(&mut buf)?),
            3 => fdp.number = Some(decode_varint(&mut buf)? as i32),
            4 => fdp.label = Some(decode_varint(&mut buf)? as i32),
            5 => fdp.r#type = Some(decode_varint(&mut buf)? as i32),
            6 => fdp.type_name = Some(decode_string(&mut buf)?),
            7 => fdp.default_value = Some(decode_string(&mut buf)?),
            8 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                fdp.options = Some(decode_field_options(msg_data)?);
            }
            9 => fdp.oneof_index = Some(decode_varint(&mut buf)? as i32),
            10 => fdp.json_name = Some(decode_string(&mut buf)?),
            17 => fdp.proto3_optional = Some(decode_varint(&mut buf)? != 0),
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(fdp)
}

/// Decode FieldOptions with protomon extensions.
fn decode_field_options(data: &[u8]) -> Result<FieldOptions, Error> {
    let mut buf = data;
    let mut opts = FieldOptions::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            // Protomon extensions
            EXT_FIELD_VEC => opts.vec = decode_varint(&mut buf)? != 0,
            EXT_FIELD_BOXED => opts.boxed = decode_varint(&mut buf)? != 0,
            EXT_FIELD_LAZY => opts.lazy = decode_varint(&mut buf)? != 0,
            EXT_FIELD_FIXED_ARRAY => opts.fixed_array = decode_varint(&mut buf)? as u32,
            // Skip all other fields (standard protobuf FieldOptions fields)
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(opts)
}

/// Decode an EnumDescriptorProto.
fn decode_enum_descriptor_proto(data: &[u8]) -> Result<EnumDescriptorProto, Error> {
    let mut buf = data;
    let mut edp = EnumDescriptorProto::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            1 => edp.name = Some(decode_string(&mut buf)?),
            2 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                edp.value.push(decode_enum_value_descriptor_proto(msg_data)?);
            }
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(edp)
}

/// Decode an EnumValueDescriptorProto.
fn decode_enum_value_descriptor_proto(data: &[u8]) -> Result<EnumValueDescriptorProto, Error> {
    let mut buf = data;
    let mut evdp = EnumValueDescriptorProto::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            1 => evdp.name = Some(decode_string(&mut buf)?),
            2 => evdp.number = Some(decode_varint(&mut buf)? as i32),
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(evdp)
}

/// Decode a OneofDescriptorProto.
fn decode_oneof_descriptor_proto(data: &[u8]) -> Result<OneofDescriptorProto, Error> {
    let mut buf = data;
    let mut odp = OneofDescriptorProto::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            1 => odp.name = Some(decode_string(&mut buf)?),
            2 => {
                let len = decode_len(&mut buf)?;
                let msg_data = &buf[..len];
                buf.advance(len);
                odp.options = Some(decode_oneof_options(msg_data)?);
            }
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(odp)
}

/// Decode OneofOptions with protomon extensions.
fn decode_oneof_options(data: &[u8]) -> Result<OneofOptions, Error> {
    let mut buf = data;
    let mut opts = OneofOptions::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            // Protomon extensions
            EXT_ONEOF_NULLABLE => opts.nullable = Some(decode_varint(&mut buf)? != 0),
            // Skip all other fields (standard protobuf OneofOptions fields)
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(opts)
}

/// Decode MessageOptions.
fn decode_message_options(data: &[u8]) -> Result<MessageOptions, Error> {
    let mut buf = data;
    let mut mo = MessageOptions::default();

    while buf.has_remaining() {
        let (field_number, wire_type) = decode_key(&mut buf)?;
        match field_number {
            7 => mo.map_entry = Some(decode_varint(&mut buf)? != 0),
            _ => skip_field(&mut buf, wire_type)?,
        }
    }

    Ok(mo)
}

/// Decode a field key (tag number + wire type).
fn decode_key(buf: &mut &[u8]) -> Result<(u32, WireType), Error> {
    let key = decode_varint(buf)?;
    let wire_type_val = (key & 0x07) as u8;
    let wire_type = WireType::try_from(wire_type_val).map_err(|_| Error::InvalidWireType(wire_type_val))?;
    let field_number = (key >> 3) as u32;
    Ok((field_number, wire_type))
}

/// Decode a varint (LEB128) with iteration limit to prevent infinite loops.
///
/// Varints can be at most 10 bytes for 64-bit values. The 10th byte can only
/// have its lowest bit set (representing bit 63 of the result).
fn decode_varint(buf: &mut &[u8]) -> Result<u64, Error> {
    let mut result: u64 = 0;
    let mut shift = 0;

    for i in 0..MAX_VARINT_BYTES {
        if !buf.has_remaining() {
            return Err(Error::UnexpectedEof);
        }
        let byte = buf.get_u8();

        // For the 10th byte (shift=63), only bit 0 can be set (bit 63 of result)
        // Any higher bits would overflow u64
        if shift == 63 && (byte & 0x7E) != 0 {
            return Err(Error::InvalidVarint);
        }

        result |= ((byte & 0x7F) as u64) << shift;

        if byte & 0x80 == 0 {
            return Ok(result);
        }

        shift += 7;

        // After 9 bytes (shift would become 63), the 10th byte is allowed
        // but it's the last one - continuation after 10 bytes is invalid
        if i == MAX_VARINT_BYTES - 1 {
            return Err(Error::InvalidVarint);
        }
    }

    // Should not reach here due to the loop structure, but handle it anyway
    Err(Error::InvalidVarint)
}

/// Decode a length value and validate it's within bounds.
fn decode_len(buf: &mut &[u8]) -> Result<usize, Error> {
    let len = decode_varint(buf)?;
    if len > MAX_MESSAGE_SIZE as u64 {
        return Err(Error::DecodeError("Message size exceeds maximum".into()));
    }
    let len = len as usize;
    if buf.remaining() < len {
        return Err(Error::UnexpectedEof);
    }
    Ok(len)
}

/// Decode a length-delimited string.
fn decode_string(buf: &mut &[u8]) -> Result<String, Error> {
    let len = decode_len(buf)?;
    // Validate UTF-8 before allocating
    let result = std::str::from_utf8(&buf[..len])
        .map_err(|_| Error::InvalidUtf8)?
        .to_string();
    buf.advance(len);
    Ok(result)
}

/// Skip a field based on its wire type.
fn skip_field(buf: &mut &[u8], wire_type: WireType) -> Result<(), Error> {
    match wire_type {
        WireType::Varint => {
            decode_varint(buf)?;
        }
        WireType::I64 => {
            if buf.remaining() < 8 {
                return Err(Error::UnexpectedEof);
            }
            buf.advance(8);
        }
        WireType::Len => {
            let len = decode_len(buf)?;
            buf.advance(len);
        }
        WireType::I32 => {
            if buf.remaining() < 4 {
                return Err(Error::UnexpectedEof);
            }
            buf.advance(4);
        }
        WireType::SGroup | WireType::EGroup => {
            // Deprecated group types - skip by returning error
            return Err(Error::InvalidWireType(wire_type as u8));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_varint() {
        // Single byte
        let mut buf: &[u8] = &[0x01];
        assert_eq!(decode_varint(&mut buf).unwrap(), 1);

        // Multi-byte (300 = 0xAC 0x02)
        let mut buf: &[u8] = &[0xAC, 0x02];
        assert_eq!(decode_varint(&mut buf).unwrap(), 300);

        // Maximum u64 value (all bits set)
        // 0xFFFFFFFFFFFFFFFF requires 10 bytes in LEB128
        let mut buf: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01];
        assert_eq!(decode_varint(&mut buf).unwrap(), u64::MAX);

        // Zero
        let mut buf: &[u8] = &[0x00];
        assert_eq!(decode_varint(&mut buf).unwrap(), 0);
    }

    #[test]
    fn test_decode_varint_overflow() {
        // 10th byte with bits 1-6 set (would overflow u64)
        let mut buf: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x02];
        assert!(matches!(decode_varint(&mut buf), Err(Error::InvalidVarint)));

        // 11 bytes (continuation bit on 10th byte)
        let mut buf: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x81, 0x00];
        assert!(matches!(decode_varint(&mut buf), Err(Error::InvalidVarint)));
    }

    #[test]
    fn test_decode_varint_eof() {
        // Empty buffer
        let mut buf: &[u8] = &[];
        assert!(matches!(decode_varint(&mut buf), Err(Error::UnexpectedEof)));

        // Truncated multi-byte varint
        let mut buf: &[u8] = &[0x80]; // continuation bit set but no more bytes
        assert!(matches!(decode_varint(&mut buf), Err(Error::UnexpectedEof)));
    }

    #[test]
    fn test_decode_string() {
        // "hello" with length prefix
        let mut buf: &[u8] = &[0x05, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(decode_string(&mut buf).unwrap(), "hello");

        // Empty string
        let mut buf: &[u8] = &[0x00];
        assert_eq!(decode_string(&mut buf).unwrap(), "");
    }

    #[test]
    fn test_decode_string_invalid_utf8() {
        // Invalid UTF-8 sequence
        let mut buf: &[u8] = &[0x02, 0xFF, 0xFE];
        assert!(matches!(decode_string(&mut buf), Err(Error::InvalidUtf8)));
    }

    #[test]
    fn test_decode_string_truncated() {
        // Length says 5 bytes but only 3 available
        let mut buf: &[u8] = &[0x05, b'h', b'e', b'l'];
        assert!(matches!(decode_string(&mut buf), Err(Error::UnexpectedEof)));
    }

    #[test]
    fn test_decode_len_too_large() {
        // Length exceeds MAX_MESSAGE_SIZE
        // 128MB encoded as varint
        let mut buf: &[u8] = &[0x80, 0x80, 0x80, 0x40];
        assert!(matches!(decode_len(&mut buf), Err(Error::DecodeError(_))));
    }

    #[test]
    fn test_decode_key() {
        // Field 1, wire type 0 (varint): key = (1 << 3) | 0 = 8
        let mut buf: &[u8] = &[0x08];
        let (field, wire) = decode_key(&mut buf).unwrap();
        assert_eq!(field, 1);
        assert_eq!(wire, WireType::Varint);

        // Field 2, wire type 2 (len): key = (2 << 3) | 2 = 18
        let mut buf: &[u8] = &[0x12];
        let (field, wire) = decode_key(&mut buf).unwrap();
        assert_eq!(field, 2);
        assert_eq!(wire, WireType::Len);
    }
}
