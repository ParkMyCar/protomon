//! Wire format for Google's Protocol Buffers, aka [protobuf](https://protobuf.dev).

/// Denotes the type of a field in an encoded protobuf message.
///
/// Protobuf messages are a series of key-value pairs. When encoded each key-value pair
/// is turned into a record consisting of a field number, a [`WireType`], and a payload.
/// The [`WireType`] indicates how large the proceeding payload is.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum WireType {
    /// Variable length integer.
    ///
    /// Used for: `int32`, `int64`, `uint32`, `uint64`, `sint32`, `sint64`, `bool`, `enum`.
    Varint = 0,
    /// 64-bit integer.
    ///
    /// Used for: `fixed64`, `sfixed64`, `double`.
    I64 = 1,
    /// Variable length field.
    ///
    /// Used for: `string`, `bytes`, `message`, packed `repeated` fields.
    Len = 2,
    /// Group start (deprecated).
    SGroup = 3,
    /// Group end (deprecated).
    EGroup = 4,
    /// 32-bit integer.
    ///
    /// Used for: `fixed32`, `sfixed32`, `float`.
    I32 = 5,
}

// N.B. It's not super important that these are the same size, but keeping them as such
// allows the compiler to make as many optimizations as possible.
crate::asserts::assert_eq_size!(WireType, Result<WireType, ()>);

impl WireType {
    /// Try to decode a [`WireType`] from the provided raw value.
    #[inline(always)]
    const fn try_from_val(value: u64) -> Result<Self, ()> {
        match value {
            0 => Ok(WireType::Varint),
            1 => Ok(WireType::I64),
            2 => Ok(WireType::Len),
            3 => Ok(WireType::SGroup),
            4 => Ok(WireType::EGroup),
            5 => Ok(WireType::I32),
            _other => Err(()),
        }
    }
}

impl TryFrom<u64> for WireType {
    type Error = ();

    #[inline(always)]
    fn try_from(value: u64) -> Result<Self, ()> {
        WireType::try_from_val(value)
    }
}
