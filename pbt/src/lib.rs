//! High-throughput property-based testing with `derive`, swarm-testing, precise sizing,
//! and full graph-theoretic type analysis over mutually inductive and uninstantiable types.

extern crate alloc;

pub mod fields;
pub mod hash;
pub mod impls;
pub mod instantiability;
pub mod memoize;
pub mod multiset;
pub mod pbt;
pub mod reflection;
pub mod scc;
pub mod size;
pub mod swarm;
pub mod unavoidability;
pub mod union_find;
