//! Helpers to assert invariants of our code.

/// Macro that asserts two types are equal in size.
macro_rules! assert_eq_size {
    ($x:ty, $y:ty) => {
        const _: fn() = || {
            let _ = core::mem::transmute::<$x, $y>;
        };
    };
}

pub(crate) use assert_eq_size;
