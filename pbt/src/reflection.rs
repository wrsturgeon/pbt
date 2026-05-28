//! Graph-theoretic analysis of the structure of algebraic data types.
//!
//! Rust types are translated s.t. a `struct` is a singleton degenerate case (one variant).

// Toposort of caching layers (top to bottom, where moving
// downward means "can access"/"can rely on"), strictly ordered to prevent cycles:
//
// - unavoidability (whether some type transitively requires a field of some other type)
// - strongly connected components (precisely identifies mutually inductive types)
// - dependencies (remove constructors from the graph by flat-mapping over field types)
// - constructors (least fixed point of removing uninstantiable variants)
// - naive variants (reading off the def'n, including uninstantiable variants)

use {
    crate::{
        hash::{map, set},
        instantiability,
        memoize::memoize,
        multiset::Multiset,
        pbt::Pbt,
        scc, unavoidability,
        union_find::{self, UnionFind},
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::{any::TypeId, iter, mem, num::NonZero},
    std::{
        collections::hash_map,
        sync::{Mutex, RwLock},
    },
    wyrand::WyRand,
};

/// All variants of each registered type
/// (transitively through dependencies),
/// *including* uninstantiable variants.
///
/// Graph-theoretically, this is a bipartite graph in which
/// types point to constructors and constructors point to types.
/// Each directed edge means "contains," i.e.
/// "has a field of this type" or "contains this variant."
static NAIVE_VARIANTS: RwLock<HashMap<TypeId, Arc<[Variant<Erased>]>>> = RwLock::new(map());

/// A type's constructors, partitioned into potential leaves and loops,
/// i.e. whether a sub-term of type `Self` is *avoidable* or *reachable*.
#[non_exhaustive]
pub(crate) struct Affordances<SelfType> {
    /// Sorted list of indices of instantiable constructors
    /// for which a sub-term of type `Self` is *avoidable*.
    pub potential_leaves: Box<[Constructor<SelfType>]>,
    /// Sorted list of indices of instantiable constructors
    /// for which a sub-term of type `Self` is *reachable*.
    pub potential_loops: Box<[Constructor<SelfType>]>,
}

/// Each variant of some type in source order.
///
/// - If this is an `enum`, each variant is one `enum` variant.
/// - if this is a `struct`, there's exactly one variant with all fields.
/// - If this is a literal type, each variant is an opaque generator.
///   For example, `usize` might have two generators:
///   *small* `usize`s (for indices) and *uniform* `usize`s.
///   Swarm testing will pick *either* one *or* the other about half the time
///   and enable both the other half of the time, so this works elegantly
///   no matter which "flavor" of `usize` we want.
#[derive(Debug)]
#[non_exhaustive]
pub struct Constructor<SelfType: ?Sized> {
    /// The index of this variant under the original source ordering.
    pub index: usize,
    /// Reflection about this constructor
    /// in terms of its original source-code variant.
    pub variant: Variant<SelfType>,
}

/// An erased type.
///
/// This type itself is uninstantiable (it's an `enum` without variants):
/// do not use it directly. Instead, `mem::transmute` and be very, very careful.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum Erased {}

/// Each variant of some type in source order.
///
/// - If this is an `enum`, each variant is one `enum` variant.
/// - if this is a `struct`, there's exactly one variant with all fields.
/// - If this is a literal type, each variant is an opaque generator.
///   For example, `usize` might have two generators:
///   *small* `usize`s (for indices) and *uniform* `usize`s.
///   Swarm testing will pick *either* one *or* the other about half the time
///   and enable both the other half of the time, so this works elegantly
///   no matter which "flavor" of `usize` we want.
#[derive(Debug)]
#[non_exhaustive]
pub enum Variant<SelfType: ?Sized> {
    /// The type of each field in this variant.
    /// Order does not matter, but total count does.
    Algebraic {
        /// The type of each field in this variant.
        /// Order does not matter, but total count does.
        field_types: Multiset<TypeId>,
    },
    /// An opaque function pointer that generates values of this type.
    Literal {
        /// An opaque function pointer that generates values of this type.
        generator: fn(&mut WyRand) -> SelfType,
    },
}

impl<SelfType> Affordances<SelfType> {
    /// Whether this type can transitively
    /// contain a field of its own type.
    ///
    /// Equivalently, whether this type can be arbitrarily large.
    ///
    /// The reason this is computed instead of cached
    /// is that swarm testing may mask some constructors,
    /// and if all inductive constructors are masked,
    /// then a masked inductive type becomes non-inductive.
    #[inline]
    #[must_use]
    pub const fn is_inductive(&self) -> bool {
        !self.potential_loops.is_empty()
    }
}

impl<T> Constructor<T> {
    /// Iterate over the types of all fields in this variant,
    /// yielding each type exactly once (skipping duplicates).
    #[inline]
    fn dedup_fields(&self) -> iter::Copied<hash_map::Keys<'_, TypeId, NonZero<usize>>> {
        self.variant.dedup_fields()
    }

    /// Iterate over the types of all fields in this variant,
    /// yielding each type exactly once (skipping duplicates).
    #[inline]
    #[must_use]
    fn fields(&self) -> &Multiset<TypeId> {
        self.variant.fields()
    }
}

impl<T> Clone for Constructor<T> {
    #[inline]
    fn clone(&self) -> Self {
        let Self { index, ref variant } = *self;
        Self {
            index,
            variant: variant.clone(),
        }
    }
}

impl<T> Variant<T> {
    /// Iterate over the types of all fields in this variant,
    /// yielding each type exactly once (skipping duplicates).
    #[inline]
    fn dedup_fields(&self) -> iter::Copied<hash_map::Keys<'_, TypeId, NonZero<usize>>> {
        const EMPTY: &HashMap<TypeId, NonZero<usize>> = &map();
        match *self {
            Self::Algebraic { ref field_types } => field_types.iter_dedup(),
            Self::Literal { .. } => EMPTY.keys(),
        }
        .copied()
    }

    /// Iterate over the types of all fields in this variant,
    /// yielding each type exactly once (skipping duplicates).
    #[inline]
    #[must_use]
    fn fields(&self) -> &Multiset<TypeId> {
        const EMPTY: &Multiset<TypeId> = &Multiset::new();
        match *self {
            Self::Algebraic { ref field_types } => field_types,
            Self::Literal { .. } => EMPTY,
        }
    }
}

impl<T> Clone for Variant<T> {
    #[inline]
    fn clone(&self) -> Self {
        match *self {
            Self::Algebraic { ref field_types } => Self::Algebraic {
                field_types: field_types.clone(),
            },
            Self::Literal { generator } => Self::Literal { generator },
        }
    }
}

/// A type's constructors, partitioned into potential leaves and loops,
/// i.e. whether a sub-term of type `Self` is *avoidable* or *reachable*.
#[inline]
pub(crate) fn affordances<T>() -> Arc<Affordances<T>>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    let erased = affordances_of(TypeId::of::<T>());
    // SAFETY: `T` is only ever the codomain of a function pointer.
    unsafe {
        mem::transmute::<
            Arc<Affordances<Erased>>, //
            Arc<Affordances<T>>,
        >(erased)
    }
}

/// A type's constructors, partitioned into potential leaves and loops,
/// i.e. whether a sub-term of type `Self` is *avoidable* or *reachable*.
#[inline]
fn affordances_of(ty: TypeId) -> Arc<Affordances<Erased>> {
    memoize!(
        "affordance" = |ty: TypeId| -> Arc<Affordances<Erased>> {
            let self_scc = scc_of(ty);
            let mut potential_leaves = vec![];
            let mut potential_loops = vec![];
            for ctor in &*constructors_of(ty) {
                let mut potential_leaf = true;
                let mut potential_loop = false;
                for field in ctor.dedup_fields() {
                    if unavoidable_of(field).contains(&ty) {
                        potential_leaf = false;
                    }
                    if scc_of(field) == self_scc {
                        potential_loop = true;
                    }
                }
                if potential_leaf {
                    let () = potential_leaves.push(ctor.clone());
                }
                // Both can coexist!
                if potential_loop {
                    let () = potential_loops.push(ctor.clone());
                }
                debug_assert!(
                    potential_leaf || potential_loop,
                    "INTERNAL ERROR (`pbt`): constructor is neither a leaf nor a loop",
                );
            }
            Arc::new(Affordances {
                potential_leaves: potential_leaves.into_boxed_slice(),
                potential_loops: potential_loops.into_boxed_slice(),
            })
        }
    )
}

/// Instantiable constructors for each type.
///
/// N.B.: A type's instantiability is as simple as `!constructors.is_empty()`.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub(crate) fn constructors_of(ty: TypeId) -> Arc<[Constructor<Erased>]> {
    static CACHE: RwLock<HashMap<TypeId, Arc<[Constructor<Erased>]>>> = RwLock::new(map());

    if let Some(cached) = CACHE
        .read()
        .expect("INTERNAL ERROR (`pbt`): instantiability lock poisoned")
        .get(&ty)
    {
        return Arc::clone(cached);
    }

    let naive = NAIVE_VARIANTS
        .read()
        .expect("INTERNAL ERROR (`pbt`): variants lock poisoned");
    let mut cache = CACHE
        .write()
        .expect("INTERNAL ERROR (`pbt`): instantiability lock poisoned");
    let () = instantiability::update(
        &naive,
        &mut cache,
        &Variant::dedup_fields,
        |index, variant| Constructor { index, variant },
    );

    Arc::clone(
        cache
            .get(&ty)
            .expect("INTERNAL ERROR (`pbt`): unregistered type during instantiability analysis"),
    )
}

/// Instantiable constructors for each type.
///
/// N.B.: A type's instantiability is as simple as `!constructors.is_empty()`.
#[inline]
#[expect(dead_code, reason = "TODO")]
fn constructors<T>() -> Arc<[Constructor<T>]>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    let erased = constructors_of(TypeId::of::<T>());
    // SAFETY: `T` is only ever the codomain of a function pointer.
    unsafe {
        mem::transmute::<
            Arc<[Constructor<Erased>]>, //
            Arc<[Constructor<T>]>,
        >(erased)
    }
}

/// Dependencies of each type: that is,
/// all types of all fields on all instatiable variants, deduplicated.
#[inline]
fn dependencies_of(ty: TypeId) -> Arc<HashSet<TypeId>> {
    memoize!(
        "dependency" = |ty: TypeId| -> Arc<HashSet<TypeId>> {
            Arc::new(
                constructors_of(ty)
                    .iter()
                    .flat_map(Constructor::dedup_fields)
                    .collect(),
            )
        }
    )
}

/// Dependencies of each type: that is,
/// all types of all fields on all instatiable variants, deduplicated.
#[inline]
#[expect(dead_code, reason = "TODO")]
fn dependencies<T>() -> Arc<HashSet<TypeId>>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    dependencies_of(TypeId::of::<T>())
}

/// All variants of a given type,
/// even uninstantiable variants.
#[inline]
#[expect(dead_code, reason = "TODO")]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn naive_variants_of(ty: TypeId) -> Arc<[Variant<Erased>]> {
    Arc::clone(
        NAIVE_VARIANTS
            .read()
            .expect("INTERNAL ERROR (`pbt`): variants lock poisoned")
            .get(&ty)
            .expect("INTERNAL ERROR (`pbt`): unregistered type"),
    )
}

/// Register the type `T` and its dependencies
/// in a naive type reflection graph,
/// including any uninstantiable variants.
#[inline]
#[expect(clippy::implicit_hasher, reason = "all in on `ahash`")]
#[expect(
    clippy::missing_panics_doc,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn register<T>(
    vertices: &mut HashMap<TypeId, Arc<[Variant<Erased>]>>,
    visited: &mut HashSet<TypeId>,
) where
    T: Pbt,
{
    // If this type has already been registered, short-circuit:
    let ty = TypeId::of::<T>();
    if vertices.contains_key(&ty) || !visited.insert(ty) {
        return;
    }

    // Recurse, i.e. run depth-first search:
    let naive_variants = T::variants(vertices, visited);

    // SAFETY: `T` is only ever the codomain of a function pointer.
    let erased = unsafe {
        mem::transmute::<
            Arc<[Variant<T>]>, //
            Arc<[Variant<Erased>]>,
        >(naive_variants)
    };

    let dup: Option<_> = vertices.insert(ty, erased);
    assert!(
        dup.is_none(),
        "INTERNAL ERROR (`pbt`): TOCTOU despite `&mut` (witchcraft)",
    );
}

/// Register the type `T` and its dependencies
/// in a naive type reflection graph,
/// including any uninstantiable variants.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn register_globally<T>()
where
    T: Pbt,
{
    if NAIVE_VARIANTS
        .read()
        .expect("INTERNAL ERROR (`pbt`): variants lock poisoned")
        .contains_key(&TypeId::of::<T>())
    {
        return;
    }

    // It's fine if there's TOCTOU, since this is
    // cheap, cached, incremental, and idempotent.
    let mut naive_variants = NAIVE_VARIANTS
        .write()
        .expect("INTERNAL ERROR (`pbt`): variants lock poisoned");

    let mut visited = set();
    let () = register::<T>(&mut naive_variants, &mut visited);
}

/// DFS of reachability between *strongly connected components* of the type-dependency graph,
/// i.e. the graph in which vertices are types and edges represent "has a field of this type."
///
/// The key insight is that *strongly connected components define mutually inductive types.*
///
/// To test reachability between *types* (not SCCs themselves),
/// first lift each type to its enclosing SCC, then test SCC reachability.
/// This is especially nice since this quotient graph is a DAG by construction,
/// so this reachability logic can safely assume the absence of cycles.
#[inline]
#[expect(dead_code, reason = "TODO")]
fn scc<T>() -> union_find::Root<TypeId, Arc<scc::QuotientVertex<TypeId>>>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    scc_of(TypeId::of::<T>())
}

/// DFS of reachability between *strongly connected components* of the type-dependency graph,
/// i.e. the graph in which vertices are types and edges represent "has a field of this type."
///
/// The key insight is that *strongly connected components define mutually inductive types.*
///
/// To test reachability between *types* (not SCCs themselves),
/// first lift each type to its enclosing SCC, then test SCC reachability.
/// This is especially nice since this quotient graph is a DAG by construction,
/// so this reachability logic can safely assume the absence of cycles.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn scc_of(ty: TypeId) -> union_find::Root<TypeId, Arc<scc::QuotientVertex<TypeId>>> {
    static QUOTIENT: Mutex<UnionFind<TypeId, Arc<scc::QuotientVertex<TypeId>>>> =
        Mutex::new(UnionFind::new());
    let mut quotient = QUOTIENT
        .lock()
        .expect("INTERNAL ERROR (`pbt`): SCC quotient graph lock poisoned");

    let () = scc::update(ty, &dependencies_of, &mut quotient);

    quotient
        .root(ty)
        .expect("INTERNAL ERROR (`pbt`): unregistered type during SCC quotienting")
}

/// Compute all raw types unavoidably reached when generating the given raw type.
///
/// A type `U` is unavoidable from `T` iff every constructor path for `T`
/// eventually requires a field whose type unavoidably reaches `U`.
/// Unavoidability is reflexive by convention: every type unavoidably reaches itself.
#[inline]
#[must_use]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn unavoidable_of(ty: TypeId) -> Arc<HashSet<TypeId>> {
    static CACHE: RwLock<HashMap<TypeId, Arc<HashSet<TypeId>>>> = RwLock::new(map());

    if let Some(cached) = CACHE
        .read()
        .expect("INTERNAL ERROR (`pbt`): unavoidability lock poisoned")
        .get(&ty)
    {
        return Arc::clone(cached);
    }

    let cache = &mut CACHE
        .write()
        .expect("INTERNAL ERROR (`pbt`): unavoidability lock poisoned");

    let () = unavoidability::update(ty, cache, &constructors_of, &Constructor::fields);

    Arc::clone(
        cache
            .get(&ty)
            .expect("INTERNAL ERROR (`pbt`): missing unavoidability result"),
    )
}

/// Compute all raw types unavoidably reached when generating the given type.
///
/// A type `U` is unavoidable from `T` iff every constructor path for `T`
/// eventually requires a field whose type unavoidably reaches `U`.
/// Unavoidability is reflexive by convention: every type unavoidably reaches itself.
#[inline]
#[must_use]
#[expect(dead_code, reason = "TODO")]
fn unavoidable<T>() -> Arc<HashSet<TypeId>>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    unavoidable_of(TypeId::of::<T>())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::fields::Fields,
        ahash::{HashMap, HashSet},
    };

    fn constructor_indices<T>(constructors: &[Constructor<T>]) -> Vec<usize> {
        constructors
            .iter()
            .map(|constructor| constructor.index)
            .collect()
    }

    fn field_types<const N: usize>(types: [TypeId; N]) -> Multiset<TypeId> {
        types.into_iter().collect()
    }

    #[test]
    fn constructor_can_be_both_potential_leaf_and_potential_loop() {
        struct A;
        struct B;

        impl Pbt for A {
            #[inline]
            fn instantiate_variant<F>(_variant_index: usize, _fields: F) -> Self
            where
                F: Fields,
            {
                Self
            }

            #[inline]
            fn variants(
                variants: &mut HashMap<TypeId, Arc<[Variant<Erased>]>>,
                visited: &mut HashSet<TypeId>,
            ) -> Arc<[Variant<Self>]> {
                let () = register::<B>(variants, visited);
                Arc::new([Variant::Algebraic {
                    field_types: field_types([TypeId::of::<B>()]),
                }])
            }
        }

        impl Pbt for B {
            #[inline]
            fn instantiate_variant<F>(_variant_index: usize, _fields: F) -> Self
            where
                F: Fields,
            {
                Self
            }

            #[inline]
            fn variants(
                variants: &mut HashMap<TypeId, Arc<[Variant<Erased>]>>,
                visited: &mut HashSet<TypeId>,
            ) -> Arc<[Variant<Self>]> {
                let () = register::<A>(variants, visited);
                Arc::new([
                    Variant::Algebraic {
                        field_types: Multiset::new(),
                    },
                    Variant::Algebraic {
                        field_types: field_types([TypeId::of::<A>()]),
                    },
                ])
            }
        }

        let affordances = affordances::<A>();

        assert_eq!(constructor_indices(&affordances.potential_leaves), vec![0]);
        assert_eq!(constructor_indices(&affordances.potential_loops), vec![0]);
    }

    #[test]
    fn literal_variants_are_distinct_leaf_constructors() {
        struct LiteralAlternatives;

        impl Pbt for LiteralAlternatives {
            #[inline]
            fn instantiate_variant<F>(_variant_index: usize, _fields: F) -> Self
            where
                F: Fields,
            {
                Self
            }

            #[inline]
            fn variants(
                _variants: &mut HashMap<TypeId, Arc<[Variant<Erased>]>>,
                _visited: &mut HashSet<TypeId>,
            ) -> Arc<[Variant<Self>]> {
                Arc::new([
                    Variant::Literal {
                        generator: |_| Self,
                    },
                    Variant::Literal {
                        generator: |_| Self,
                    },
                ])
            }
        }

        let affordances = affordances::<LiteralAlternatives>();

        assert_eq!(
            constructor_indices(&affordances.potential_leaves),
            vec![0, 1],
        );
        assert_eq!(
            constructor_indices(&affordances.potential_loops),
            Vec::<usize>::new(),
        );
    }
}
