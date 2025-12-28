#![no_std]
#![deny(clippy::as_conversions)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod codec;
pub mod error;
pub mod leb128;
pub mod wire;

mod util;

#[cfg(feature = "derive")]
pub use protomon_derive::{ProtoMessage, ProtoOneof};
