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

#[inline(always)]
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
