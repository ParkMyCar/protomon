//! Helpers to assert invariants of our code and safe casting utilities.
//!
//! # Casting
//!
//! This module provides type-safe alternatives to Rust's `as` operator for
//! numeric conversions.
//!
//! - [`CastFrom`]: Infallible casts that are always safe (e.g. u32 → u64).
//! - [`TruncatingCastFrom`]: Casts that may truncate (e.g. u64 → u32).
//! - [`ReinterpretCastFrom`]: Casts that reinterpret the bytes as a new type (e.g. u64 -> i64).

/// Macro that asserts two types are equal in size.
macro_rules! assert_eq_size {
    ($x:ty, $y:ty) => {
        const _: fn() = || {
            let _ = core::mem::transmute::<$x, $y>;
        };
    };
}

pub(crate) use assert_eq_size;

#[inline(never)]
#[cold]
fn cold_path() {}

/// "Annotation" to hint that a branch of an if-statement is likely to occur.
#[inline(always)]
pub(crate) fn likely(b: bool) -> bool {
    if b {
        true
    } else {
        cold_path();
        false
    }
}

/// "Annotation" to hint that a branch of an if-statement is _not likely_ to occur.
#[inline(always)]
pub(crate) fn unlikely(b: bool) -> bool {
    if b {
        cold_path();
        true
    } else {
        false
    }
}

/// Wrapper to align data to a 64-byte cache line boundary.
#[repr(C, align(64))]
pub(crate) struct CacheAligned<T>(pub T);

/// Infallible cast from type `T` to `Self`.
///
/// This trait is implemented for conversions that are always safe, such as
/// widening conversions (u32 → u64) or platform-guaranteed conversions.
pub trait CastFrom<T> {
    /// Performs the cast.
    fn cast_from(from: T) -> Self;
}

/// Explicit truncating cast from type `T` to `Self`.
///
/// This trait is implemented for conversions that may truncate data. Using
/// this trait makes it explicit that truncation is intentional.
pub trait TruncatingCastFrom<T>: Sized {
    /// Performs the cast, potentially truncating the value.
    fn truncating_cast_from(from: T) -> Self;
}

/// Bit reinterpretation cast from type `T` to `Self`.
///
/// This trait is for same-size signed/unsigned conversions where we want to
/// reinterpret the bits without any value conversion (e.g., u64 -> i64).
pub trait ReinterpretCastFrom<T>: Sized {
    /// Reinterprets the bits of `from` as `Self`.
    fn reinterpret_cast_from(from: T) -> Self;
}

macro_rules! impl_cast_from {
    ($from:ty => $to:ty) => {
        impl CastFrom<$from> for $to {
            #[allow(clippy::as_conversions)]
            #[inline(always)]
            fn cast_from(from: $from) -> Self {
                from as Self
            }
        }

        impl TruncatingCastFrom<$from> for $to {
            #[allow(clippy::as_conversions)]
            #[inline(always)]
            fn truncating_cast_from(from: $from) -> Self {
                from as Self
            }
        }
    };
}

macro_rules! impl_truncating_cast_from {
    ($from:ty => $to:ty) => {
        impl TruncatingCastFrom<$from> for $to {
            #[allow(clippy::as_conversions)]
            #[inline(always)]
            fn truncating_cast_from(from: $from) -> Self {
                from as Self
            }
        }
    };
}

macro_rules! impl_reinterpret_cast {
    ($from:ty => $to:ty) => {
        impl ReinterpretCastFrom<$from> for $to {
            #[allow(clippy::as_conversions)]
            #[inline(always)]
            fn reinterpret_cast_from(from: $from) -> Self {
                from as Self
            }
        }
    };
}

impl_cast_from!(u8 => u8);
impl_cast_from!(u8 => u16);
impl_cast_from!(u8 => u32);
impl_cast_from!(u8 => u64);
impl_cast_from!(u8 => u128);
impl_cast_from!(u16 => u16);
impl_cast_from!(u16 => u32);
impl_cast_from!(u16 => u64);
impl_cast_from!(u16 => u128);
impl_cast_from!(u32 => u32);
impl_cast_from!(u32 => u64);
impl_cast_from!(u32 => u128);
impl_cast_from!(u64 => u64);
impl_cast_from!(u64 => u128);

impl_cast_from!(i8 => i16);
impl_cast_from!(i8 => i32);
impl_cast_from!(i8 => i64);
impl_cast_from!(i8 => i128);
impl_cast_from!(i16 => i32);
impl_cast_from!(i16 => i64);
impl_cast_from!(i16 => i128);
impl_cast_from!(i32 => i64);
impl_cast_from!(i32 => i128);
impl_cast_from!(i64 => i128);

impl_cast_from!(f32 => f64);

// On 64-bit platforms, usize is 64 bits.
#[cfg(target_pointer_width = "64")]
mod platform {
    use super::*;

    // Infallible, smaller types always fit in usize.
    impl_cast_from!(u8 => usize);
    impl_cast_from!(i8 => isize);
    impl_cast_from!(u16 => usize);
    impl_cast_from!(i16 => isize);
    impl_cast_from!(u32 => usize);
    impl_cast_from!(i32 => isize);

    // Infallible, u64 and usize are the same on 64-bit.
    impl_cast_from!(u64 => usize);
    impl_cast_from!(i64 => isize);
    impl_cast_from!(usize => u64);
    impl_cast_from!(isize => i64);

    // Infallible, usize fits in u128.
    impl_cast_from!(usize => u128);
    impl_cast_from!(isize => i128);

    impl_truncating_cast_from!(usize => u32);
    impl_truncating_cast_from!(isize => i32);
}

// On 32-bit platforms, usize is 32 bits
#[cfg(target_pointer_width = "32")]
mod platform {
    use super::*;

    // Infallible, smaller types always fit in usize.
    impl_cast_from!(u8 => usize);
    impl_cast_from!(i8 => isize);
    impl_cast_from!(u16 => usize);
    impl_cast_from!(i16 => isize);

    // Infallible, u32 fits in usize on 32-bit.
    impl_cast_from!(u32 => usize);
    impl_cast_from!(i32 => isize);
    impl_cast_from!(usize => u32);
    impl_cast_from!(isize => i32);

    // Infallible, usize fits in u64 on 32-bit.
    impl_cast_from!(usize => u64);
    impl_cast_from!(isize => i64);

    impl_truncating_cast_from!(u64 => usize);
    impl_truncating_cast_from!(i64 => isize);
}

// Narrowing unsigned
impl_truncating_cast_from!(u16 => u8);
impl_truncating_cast_from!(u32 => u8);
impl_truncating_cast_from!(u64 => u8);
impl_truncating_cast_from!(u128 => u8);
impl_truncating_cast_from!(u32 => u16);
impl_truncating_cast_from!(u64 => u16);
impl_truncating_cast_from!(u128 => u16);
impl_truncating_cast_from!(u64 => u32);
impl_truncating_cast_from!(u128 => u32);
impl_truncating_cast_from!(u128 => u64);

// Narrowing signed
impl_truncating_cast_from!(i16 => i8);
impl_truncating_cast_from!(i32 => i8);
impl_truncating_cast_from!(i64 => i8);
impl_truncating_cast_from!(i128 => i8);
impl_truncating_cast_from!(i32 => i16);
impl_truncating_cast_from!(i64 => i16);
impl_truncating_cast_from!(i128 => i16);
impl_truncating_cast_from!(i64 => i32);
impl_truncating_cast_from!(i128 => i32);
impl_truncating_cast_from!(i128 => i64);

// Same-size signed/unsigned reinterpretation casts
impl_reinterpret_cast!(u8 => i8);
impl_reinterpret_cast!(i8 => u8);
impl_reinterpret_cast!(u16 => i16);
impl_reinterpret_cast!(i16 => u16);
impl_reinterpret_cast!(u32 => i32);
impl_reinterpret_cast!(i32 => u32);
impl_reinterpret_cast!(u64 => i64);
impl_reinterpret_cast!(i64 => u64);
impl_reinterpret_cast!(u128 => i128);
impl_reinterpret_cast!(i128 => u128);
impl_reinterpret_cast!(usize => isize);
impl_reinterpret_cast!(isize => usize);
