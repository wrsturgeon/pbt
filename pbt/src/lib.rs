//! Property-based testing from explicit type-former descriptions.
//!
//! The crate models each [`pbt::Pbt`] type as either a literal generator/shrinker
//! or an algebraic type with constructors and one eliminator. Search uses that
//! reflection data to generate values, detect uninstantiable recursive shapes,
//! and shrink counterexamples structurally.

extern crate alloc;

/// Persistent witness cache support.
pub mod cache;
mod impls;
/// Hash-based finite multisets.
pub mod multiset;
/// Ordered finite multisets.
pub mod ordered_multiset;
/// Core `Pbt` trait, type formers, constructors, eliminators, and generation.
pub mod pbt;
/// Global type registry and erased reflection metadata.
pub mod reflection;
mod scc;
/// Public witness search and assertion helpers.
pub mod search;
/// Structural shrinking.
pub mod shrink;
/// Dependent-pair style filtered values.
pub mod sigma;
/// Generation-size accounting.
pub mod size;

#[cfg(test)]
mod test;

// Re-exports for macros:
pub use {pbt_macros::Pbt, scc::StronglyConnectedComponents, wyrand::WyRand};

/// The 16-bit hash seed, to be zero-extended for various platforms.
pub const SEED: u16 = 0x1337;
