//! Implementations for common types
//! that would otherwise fall to the orphan rule.

mod arcs;
mod arrays;
mod booleans;
mod boxes;
mod chars;
mod hash_collections;
mod infallible;
mod integers;
#[cfg(feature = "serde_json")]
mod json;
mod options;
mod phantoms;
mod strings;
mod tuples;
mod vectors;
