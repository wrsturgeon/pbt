pub mod construct;
mod impls;
pub mod multiset;
pub mod reflection;
pub mod search;
pub mod shrink;
pub mod size;

#[cfg(test)]
mod test;

// Re-exports for macros:
pub use {pbt_macros::Pbt, wyrand::WyRand};

/// The 16-bit hash seed, to be zero-extended for various platforms.
pub const SEED: u16 = 0x1337;
