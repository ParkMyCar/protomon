#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod codec;
pub mod error;
pub mod leb128;
mod util;
pub mod wire;

#[cfg(feature = "derive")]
pub use protomon_derive::{ProtoMessage, ProtoOneof};
