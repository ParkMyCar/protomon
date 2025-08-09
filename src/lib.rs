mod error;
mod wire;

// Publically export `buffer` so users can implement the relevant traits.
pub mod buffer;
// Publically export `leb128` because the functions are useful on their own.
pub mod leb128;
