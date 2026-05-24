//! Graph-theoretic analysis of the structure of algebraic data types.
//!
//! Rust types are translated s.t. a `struct` is a singleton degenerate case (one variant).

use {
    crate::{hash::map, multiset::Multiset, pbt::Pbt, type_id::Type, union_find::UnionFind},
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::any,
    std::sync::RwLock,
};

/// Every registered type and its directed edges (representing "has a field of this type"),
/// without any non-local graph-theoretic information.
///
/// This graph may be cyclic (and it often will be, exactly when there exists an inductive type),
/// but incoming edges will never affect inductive logic unless it closes a loop,
/// so graph-theoretic analyses are free to care only about vertices reachable from some type.
///
/// The above realization also leads to the following insight:
/// *strongly connected components define mutually inductive types.*
pub static VERTICES: RwLock<HashMap<Type, AlgebraicDataType>> = RwLock::new(map());

/// DFS of reachability between *strongly connected components* of the type-dependency graph,
/// i.e. the graph in which vertices are types and edges represent "has a field of this type."
///
/// The key insight is that *strongly connected components define mutually inductive types.*
///
/// To test reachability between *types* (not SCCs themselves),
/// first lift each type to its enclosing SCC, then test SCC reachability.
/// This is especially nice since this quotient graph is a DAG by construction,
/// so this reachability logic can safely assume the absence of cycles.
pub static QUOTIENT: RwLock<UnionFind<Scc, Arc<SccQuotientVertex>>> = RwLock::new(UnionFind::new());

/// Transitive reachability between *strongly connected components* of the type-dependency graph,
/// i.e. the graph in which vertices are types and edges represent "has a field of this type."
///
/// The key insight is that *strongly connected components define mutually inductive types.*
///
/// This is useful when determining whether some variant is potentially inductive:
/// if any of its fields can reach `Self`, then it can potentially be inductive,
/// so we should consider it when we have plenty of size left to generate.
pub static REACHABLE: RwLock<HashMap<Scc, HashSet<Scc>>> = RwLock::new(map());

/// This encodes the notion that "if we start generating a term of type A,
/// then must unavoidably generate terms of type B, C, ... in the process."
///
/// This is useful when determining whether some variant could be a leaf:
/// if any of its fields unavoidably reach `Self`, then it can never be a leaf,
/// so we should avoid it when we're running out of size remaining to generate.
pub static UNAVOIDABLE: RwLock<HashMap<Type, HashSet<Type>>> = RwLock::new(map());

/// Runtime representation of the structure of an algebraic data type.
///
/// A `struct` is a singleton degenerate case of an `enum`;
/// viewed differently, an `enum` is a collection of `struct`s.
#[non_exhaustive]
pub struct AlgebraicDataType {
    /// The name of this type, as acquired by `core::any::type_name`.
    pub name: &'static str,
    /// All types of all fields of all variants.
    pub potential_fields: HashSet<Type>,
    /// Type-level reflection: variants, field types, erased trait operations, etc.
    pub reflection: Reflection,
}

/// Type-level reflection: variants, field types, erased trait operations, etc.
#[non_exhaustive]
pub struct Reflection {
    /// Each variant of this type, in source order.
    pub variants: Box<[Variant]>,
}

/// A thin wrapper around `Type` to indicate that
/// this *individual* type should be discarded
/// as merely an index into the SCC of which is it an element.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Scc(Type);

/// All fields of all types within an SCC (i.e. a mutually inductive set of types)
/// that do not themselves belong to the SCC.
///
/// Recall that fields of each individual type are directed edges,
/// so directed edges out of an SCC are not a very well-defined concept,
/// but they could be seen as representing "optional dependencies,"
/// i.e. that there exists a generator path that contains a term of this type.
#[non_exhaustive]
pub struct SccQuotientVertex {
    /// All elements of this strongly connected component,
    /// i.e. all types mutually inductive w.r.t. each other.
    pub elements: HashSet<Type>,

    /// All fields of all types within an SCC (i.e. a mutually inductive set of types)
    /// that do not themselves belong to the SCC.
    ///
    /// Recall that fields of each individual type are directed edges,
    /// so directed edges out of an SCC are not a very well-defined concept,
    /// but they could be seen as representing "optional dependencies,"
    /// i.e. that there exists a generator path that contains a term of this type.
    pub immediately_reachable: HashSet<Scc>,
}

/// One variant of an `enum`, or all fields on a `struct`.
///
/// A `struct` is a singleton degenerate case of an `enum`;
/// viewed differently, an `enum` is a collection of `struct`s.
#[non_exhaustive]
pub struct Variant {
    /// The types of each field in this variant.
    /// Order does not matter, but total count does.
    pub fields: Multiset<Type>,
}

impl AlgebraicDataType {
    /// Analyze the type `T` given that `variants` accurately represents its variants.
    ///
    /// A `struct` is a singleton degenerate case of an `enum` (use a single variant);
    /// viewed differently, an `enum` is a collection of `struct`s.
    #[inline]
    #[must_use]
    pub fn new<T>(reflection: Reflection) -> Self
    where
        T: ?Sized,
    {
        Self {
            name: any::type_name::<T>(),
            potential_fields: reflection
                .variants
                .iter()
                .flat_map(|variant| variant.fields.counts.keys().copied())
                .collect(),
            reflection,
        }
    }
}

/// Register the type `T` in the global type reflection graph.
///
/// Note that this does *not* include `T` in any strongly connected components logic
/// or any higher graph-theoretic operations. This merely registers the type for later use.
///
/// # Panics
///
/// If the reflection registry lock has been poisoned:
/// i.e. if *another* process *already* panicked while holding the lock.
#[inline]
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn register<T>()
where
    T: Pbt,
{
    let mut vertices = VERTICES
        .write()
        .expect("INTERNAL ERROR (`pbt`): reflection registry lock poisoned");
    let ty = Type::new::<T>();
    if !vertices.contains_key(&ty)
        && vertices
            .insert(ty, AlgebraicDataType::new::<T>(T::reflect()))
            .is_some()
    {
        panic!("INTERNAL ERROR (`pbt`): TOCTOU despite `&mut` (some witchcraft going on)")
    }
}
