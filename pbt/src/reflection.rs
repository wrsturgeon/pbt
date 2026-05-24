//! Graph-theoretic analysis of the structure of algebraic data types.
//!
//! Rust types are translated s.t. a `struct` is a singleton degenerate case (one variant).

use {
    crate::{
        hash::map,
        multiset::Multiset,
        pbt::Pbt,
        scc,
        type_id::Type,
        union_find::{RootElement, UnionFind},
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::any,
    std::sync::Mutex,
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
static VERTICES: Mutex<HashMap<Type, Arc<AlgebraicDataType>>> = Mutex::new(map());

/// DFS of reachability between *strongly connected components* of the type-dependency graph,
/// i.e. the graph in which vertices are types and edges represent "has a field of this type."
///
/// The key insight is that *strongly connected components define mutually inductive types.*
///
/// To test reachability between *types* (not SCCs themselves),
/// first lift each type to its enclosing SCC, then test SCC reachability.
/// This is especially nice since this quotient graph is a DAG by construction,
/// so this reachability logic can safely assume the absence of cycles.
static QUOTIENT: Mutex<UnionFind<Type, Arc<scc::QuotientVertex<Type>>>> =
    Mutex::new(UnionFind::new());

/// Transitive reachability between *strongly connected components* of the type-dependency graph,
/// i.e. the graph in which vertices are types and edges represent "has a field of this type."
///
/// The key insight is that *strongly connected components define mutually inductive types.*
///
/// This is useful when determining whether some variant is potentially inductive:
/// if any of its fields can reach `Self`, then it can potentially be inductive,
/// so we should consider it when we have plenty of size left to generate.
static REACHABLE: Mutex<HashMap<RootElement<Type>, Arc<HashSet<RootElement<Type>>>>> =
    Mutex::new(map());

/// This encodes the notion that "if we start generating a term of type A,
/// then must unavoidably generate terms of type B, C, ... in the process."
///
/// This is useful when determining whether some variant could be a leaf:
/// if any of its fields unavoidably reach `Self`, then it can never be a leaf,
/// so we should avoid it when we're running out of size remaining to generate.
static UNAVOIDABLE: Mutex<HashMap<Type, Arc<HashSet<Type>>>> = Mutex::new(map());

/// Runtime representation of the structure of an algebraic data type.
///
/// A `struct` is a singleton degenerate case of an `enum`;
/// viewed differently, an `enum` is a collection of `struct`s.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reflection {
    /// Each variant of this type, in source order.
    pub variants: Box<[Variant]>,
}

/// One variant of an `enum`, or all fields on a `struct`.
///
/// A `struct` is a singleton degenerate case of an `enum`;
/// viewed differently, an `enum` is a collection of `struct`s.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
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

/// Query reflection for the given type ID iff it is *immediately* available.
///
/// If the type graph is currently locked, no matter how briefly, this will fail.
#[inline]
pub fn get_immediate(ty: &Type) -> Option<Arc<AlgebraicDataType>> {
    let map = VERTICES.try_lock().ok()?;
    let adt = map.get(ty)?;
    Some(Arc::clone(adt))
}

/// Compute all reachable SCCs from the given SCC,
/// transitively caching all results.
///
/// N.B.: since the SCC quotient graph is a DAG by construction,
/// we arbitrarily consider reachability to be reflexive w.l.o.g.
/// (i.e. all SCCs can reach themselves)
/// to simplify downstream reachability checks.
///
/// N.B.: This function locks `REACHABLE` and `QUOTIENT`, in that order.
#[inline]
#[expect(
    clippy::expect_used,
    clippy::missing_panics_doc,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn reachable(scc_root: RootElement<Type>) -> Arc<HashSet<RootElement<Type>>> {
    scc::reachable(
        &mut REACHABLE
            .lock()
            .expect("INTERNAL ERROR (`pbt`): graph reachability lock poisoned"),
        &mut QUOTIENT
            .lock()
            .expect("INTERNAL ERROR (`pbt`): quotient graph lock poisoned"),
        scc_root,
    )
}

/// Compute all raw types unavoidably reached when generating the given raw type.
///
/// A type `U` is unavoidable from `T` iff every constructor path for `T`
/// eventually requires a field whose type unavoidably reaches `U`.
/// Unavoidability is reflexive by convention: every type unavoidably reaches itself.
///
/// N.B.: This function locks `UNAVOIDABLE`, `VERTICES`, and `QUOTIENT`, in that order.
#[inline]
#[expect(
    clippy::expect_used,
    clippy::missing_panics_doc,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn unavoidable(ty: Type) -> Arc<HashSet<Type>> {
    let unavoidable = &mut UNAVOIDABLE
        .lock()
        .expect("INTERNAL ERROR (`pbt`): graph unavoidability lock poisoned");
    let vertices = VERTICES
        .lock()
        .expect("INTERNAL ERROR (`pbt`): reflection registry lock poisoned");
    let mut quotient = QUOTIENT
        .lock()
        .expect("INTERNAL ERROR (`pbt`): quotient graph lock poisoned");
    let () = scc::update_unavoidable(
        ty,
        unavoidable,
        &vertices,
        &mut quotient,
        &|adt: &Arc<AlgebraicDataType>| adt.reflection.variants.as_ref(),
        &|variant: &Variant| &variant.fields,
    );

    Arc::clone(
        unavoidable
            .get(&ty)
            .expect("INTERNAL ERROR (`pbt`): missing requested unavoidability result"),
    )
}

/// Register the type `T` in the global type reflection graph.
///
/// Note that this does *not* include `T` in any strongly connected components logic
/// or any higher graph-theoretic operations. This merely registers the type for later use.
#[inline]
#[expect(clippy::implicit_hasher, reason = "all in on `ahash`")]
#[expect(
    clippy::missing_panics_doc,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn register<T>(
    vertices: &mut HashMap<Type, Arc<AlgebraicDataType>>,
    visited: &mut HashSet<Type>,
) where
    T: Pbt,
{
    // If this type has already been registered, short-circuit:
    let ty = Type::new::<T>();
    if vertices.contains_key(&ty) || !visited.insert(ty) {
        return;
    }

    // Recurse, i.e. run depth-first search,
    // and also gather type reflection data:
    let reflection = T::reflect(vertices, visited);

    // Insert the recursive result:
    assert_eq!(
        vertices.insert(ty, Arc::new(AlgebraicDataType::new::<T>(reflection))),
        None,
        "INTERNAL ERROR (`pbt`): TOCTOU during DFS",
    );
}
