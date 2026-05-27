//! Graph-theoretic analysis of the structure of algebraic data types.
//!
//! Rust types are translated s.t. a `struct` is a singleton degenerate case (one variant).

use {
    crate::{
        graph::update_instantiability,
        hash::{map, set},
        multiset::Multiset,
        pbt::Pbt,
        scc,
        union_find::{self, RootElement, UnionFind},
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::{
        any::{self, TypeId},
        iter, mem,
        num::NonZero,
    },
    std::{
        collections::hash_map,
        sync::{Mutex, RwLock},
    },
    wyrand::WyRand,
};

/// Cache an *idempotent* calculation with an `RwLock<HashMap<K, V>>`.
#[expect(
    unused_macros,
    reason = "TODO: evaluate after reflection caches settle"
)]
macro_rules! cache {
    ($name:literal = |$k:ident: $K:ty| -> $V:ty $b:block) => {{
        static CACHE: RwLock<HashMap<$K, $V>> = RwLock::new(map());

        // Check if this key already has a cached result:
        {
            let read = CACHE.read().expect(concat!(
                "INTERNAL ERROR (`pbt`): ",
                $name,
                " cache lock poisoned",
            ));
            if let Some(cached) = read.get(&$k) {
                return <$V as Clone>::clone(cached);
            }
        }

        // Otherwise, compute the result and insert it,
        // unless there was a race condition (in which case
        // it's important that this function be idempotent):
        let v = $b;
        let mut write = CACHE.write().expect(concat!(
            "INTERNAL ERROR (`pbt`): ",
            $name,
            " cache lock poisoned",
        ));
        <$V as Clone>::clone(write.entry($k).or_insert(v))
    }};
}

/// All variants of each registered type
/// (transitively through dependencies),
/// *including* uninstantiable variants.
///
/// Graph-theoretically, this is a bipartite graph in which
/// types point to constructors and constructors point to types.
/// Each directed edge means "contains," i.e.
/// "has a field of this type" or "contains this variant."
static NAIVE_VARIANTS: RwLock<HashMap<TypeId, Arc<[Variant<Erased>]>>> = RwLock::new(map());

/// Transitive reachability between *strongly connected components* of the type-dependency graph,
/// i.e. the graph in which vertices are types and edges represent "has a field of this type."
///
/// The key insight is that *strongly connected components define mutually inductive types.*
///
/// This is useful when determining whether some variant is potentially inductive:
/// if any of its fields can reach `Self`, then it can potentially be inductive,
/// so we should consider it when we have plenty of size left to generate.
#[expect(dead_code, reason = "TODO")]
static REACHABLE: RwLock<HashMap<RootElement<TypeId>, Arc<HashSet<RootElement<TypeId>>>>> =
    RwLock::new(map());

/// This encodes the notion that "if we start generating a term of type A,
/// then must unavoidably generate terms of type B, C, ... in the process."
///
/// This is useful when determining whether some variant could be a leaf:
/// if any of its fields unavoidably reach `Self`, then it can never be a leaf,
/// so we should avoid it when we're running out of size remaining to generate.
#[expect(dead_code, reason = "TODO")]
static UNAVOIDABLE: RwLock<HashMap<TypeId, Arc<HashSet<TypeId>>>> = RwLock::new(map());

/// The "final boss" of graph analysis, containing
/// precomputed data structures for efficient generation.
#[expect(dead_code, reason = "TODO")]
static AFFORDANCES: RwLock<HashMap<TypeId, Affordances<Erased>>> = RwLock::new(map());

/// An erased type.
///
/// This type itself is uninstantiable (it's an `enum` without variants):
/// do not use it directly. Instead, `mem::transmute` and be very, very careful.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum Erased {}

/// A type's constructors,
/// partitioned into potential leaves and loops.
#[non_exhaustive]
pub struct Affordances<SelfType> {
    /// All constructors of this type in source order.
    ///
    /// DO NOT sample over `constructors` directly.
    /// Instead, sample `potential_loops` or `potential_leaves`,
    /// then use that index here to query the associated constructor.
    pub constructors: Arc<[Variant<SelfType>]>,
    /// Sorted lists of indices of instantiable constructors for which
    /// a sub-term of type `Self` is either *avoidable* or *reachable*.
    pub leaves_and_loops: LeavesAndLoops,
}

/// Sorted lists of constructor indices for which
/// a sub-term of type `Self` is *avoidable* or *reachable*.
#[non_exhaustive]
pub struct LeavesAndLoops {
    /// Sorted list of indices of instantiable constructors
    /// for which a sub-term of type `Self` is *avoidable*.
    pub potential_leaves: Box<[usize]>,
    /// Sorted list of indices of instantiable constructors
    /// for which a sub-term of type `Self` is *reachable*.
    pub potential_loops: Box<[usize]>,
}

/// Type-level reflection: variants, field types, erased trait operations, etc.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Reflection<SelfType: ?Sized> {
    /// Each variant of this type in source order.
    ///
    /// - If this is an `enum`, each variant is one `enum` variant.
    /// - if this is a `struct`, there's exactly one variant with all fields.
    /// - If this is a literal type, each variant is an opaque generator.
    ///   For example, `usize` might have two generators:
    ///   *small* `usize`s (for indices) and *uniform* `usize`s.
    ///   Swarm testing will pick *either* one *or* the other about half the time
    ///   and enable both the other half of the time, so this works elegantly
    ///   no matter which "flavor" of `usize` we want.
    pub variants: Arc<[Variant<SelfType>]>,
}

/// Runtime representation of the structure of an algebraic data type.
///
/// A `struct` is a singleton degenerate case of an `enum`;
/// viewed differently, an `enum` is a collection of `struct`s.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct TypeGraphVertex {
    /// The name of this type, as acquired by `core::any::type_name`.
    pub name: &'static str,
    /// All types of all fields of all variants.
    pub potential_fields: HashSet<TypeId>,
    /// Type-level reflection: variants, field types, erased trait operations, etc.
    pub reflection: Reflection<Erased>,
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
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum Variant<SelfType: ?Sized> {
    /// The type of each field in this variant.
    /// Order does not matter, but total count does.
    Algebraic {
        /// The type of each field in this variant.
        /// Order does not matter, but total count does.
        fields: Multiset<TypeId>,
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
        self.leaves_and_loops.is_inductive()
    }
}

impl LeavesAndLoops {
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

impl<T: ?Sized> Reflection<T> {
    /// Erase the type that originally created this reflection.
    ///
    /// This is extremely dangerous.
    /// You must be sure you statically know the type with which
    /// to use the erased function after transmuting it back in the future.
    #[inline]
    const fn erase(self) -> Reflection<Erased> {
        // SAFETY: `T` is only ever the codomain of a function pointer.
        unsafe {
            mem::transmute::<
                Reflection<T>, // i.e. `Self`
                Reflection<Erased>,
            >(self)
        }
    }
}

impl TypeGraphVertex {
    /// Analyze the type `T` given that `variants` accurately represents its variants.
    ///
    /// A `struct` is a singleton degenerate case of an `enum` (use a single variant);
    /// viewed differently, an `enum` is a collection of `struct`s.
    #[inline]
    #[must_use]
    pub fn new<T>(reflection: Reflection<T>) -> Self
    where
        T: ?Sized,
    {
        Self {
            name: any::type_name::<T>(),
            potential_fields: {
                let mut acc = set();
                for variant in &*reflection.variants {
                    if let Variant::Algebraic { ref fields } = *variant {
                        let () = acc.extend(fields.counts.keys().copied());
                    }
                }
                acc
            },
            reflection: reflection.erase(),
        }
    }
}

impl<T> Variant<T> {
    /// Iterate over the types of all fields in this variant,
    /// yielding each type exactly once (skipping duplicates).
    #[inline]
    pub fn dedup_fields(&self) -> iter::Copied<hash_map::Keys<'_, TypeId, NonZero<usize>>> {
        const EMPTY: &HashMap<TypeId, NonZero<usize>> = &map();
        match *self {
            Self::Algebraic { ref fields } => fields.iter_dedup(),
            Self::Literal { .. } => EMPTY.keys(),
        }
        .copied()
    }

    /// Iterate over the types of all fields in this variant,
    /// yielding each type exactly once (skipping duplicates).
    #[inline]
    #[must_use]
    pub fn fields(&self) -> &Multiset<TypeId> {
        const EMPTY: &Multiset<TypeId> = &Multiset::new();
        match *self {
            Self::Algebraic { ref fields } => fields,
            Self::Literal { .. } => EMPTY,
        }
    }
}

/// Instantiable constructors for each type.
///
/// N.B.: A type's instantiability is as simple as `!constructors.is_empty()`.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn constructors_of(ty: TypeId) -> Arc<[Variant<Erased>]> {
    static CACHE: RwLock<HashMap<TypeId, Arc<[Variant<Erased>]>>> = RwLock::new(map());

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
    let () = update_instantiability(&naive, &mut cache, &Variant::dedup_fields);

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
fn constructors<T>() -> Arc<[Variant<T>]>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    let erased = constructors_of(TypeId::of::<T>());
    // SAFETY: `T` is only ever the codomain of a function pointer.
    unsafe {
        mem::transmute::<
            Arc<[Variant<Erased>]>, //
            Arc<[Variant<T>]>,
        >(erased)
    }
}

/// Dependencies of each type: that is,
/// all types of all fields on all instatiable variants, deduplicated.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn dependencies_of(ty: TypeId) -> Arc<HashSet<TypeId>> {
    static CACHE: RwLock<HashMap<TypeId, Arc<HashSet<TypeId>>>> = RwLock::new(map());

    if let Some(cached) = CACHE
        .read()
        .expect("INTERNAL ERROR (`pbt`): instantiability lock poisoned")
        .get(&ty)
    {
        return Arc::clone(cached);
    }

    let deps = constructors_of(ty)
        .iter()
        .flat_map(Variant::dedup_fields)
        .collect();

    let mut cache = CACHE
        .write()
        .expect("INTERNAL ERROR (`pbt`): instantiability lock poisoned");

    Arc::clone(cache.entry(ty).or_insert(Arc::new(deps)))
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

/// Compute all reachable SCCs from the given SCC,
/// transitively caching all results.
///
/// N.B.: since the SCC quotient graph is a DAG by construction,
/// we arbitrarily consider reachability to be reflexive w.l.o.g.
/// (i.e. all SCCs can reach themselves)
/// to simplify downstream reachability checks.
///
/// N.B.: This function locks `REACHABLE` and `QUOTIENT` in that order.
#[inline]
#[must_use]
#[expect(clippy::todo, reason = "TODO")]
pub fn reachable(_scc_root: RootElement<TypeId>) -> Arc<HashSet<RootElement<TypeId>>> {
    todo!()
    // scc::reachable(
    //     &mut REACHABLE
    //         .write()
    //         .expect("INTERNAL ERROR (`pbt`): graph reachability lock poisoned"),
    //     &mut SCC_QUOTIENT
    //         .lock()
    //         .expect("INTERNAL ERROR (`pbt`): quotient graph lock poisoned"),
    //     scc_root,
    // )
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
#[expect(
    clippy::expect_used,
    clippy::missing_panics_doc,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn scc(ty: TypeId) -> union_find::Root<TypeId, Arc<scc::QuotientVertex<TypeId>>> {
    static QUOTIENT: Mutex<UnionFind<TypeId, Arc<scc::QuotientVertex<TypeId>>>> =
        Mutex::new(UnionFind::new());
    let mut quotient = QUOTIENT
        .lock()
        .expect("INTERNAL ERROR (`pbt`): SCC quotient graph lock poisoned");

    let () = scc::update_quotient_reachable_from(ty, &dependencies_of, &mut quotient);

    quotient
        .root(ty)
        .expect("INTERNAL ERROR (`pbt`): unregistered type during SCC quotienting")
}

/// Compute all raw types unavoidably reached when generating the given raw type.
///
/// A type `U` is unavoidable from `T` iff every constructor path for `T`
/// eventually requires a field whose type unavoidably reaches `U`.
/// Unavoidability is reflexive by convention: every type unavoidably reaches itself.
///
/// N.B.: This function locks `UNAVOIDABLE`, `QUOTIENT`, and `CONSTRUCTORS` in that order.
#[inline]
#[must_use]
#[expect(clippy::todo, reason = "TODO")]
pub fn unavoidable(_ty: TypeId) -> Arc<HashSet<TypeId>> {
    todo!()
    // let unavoidable = &mut UNAVOIDABLE
    //     .write()
    //     .expect("INTERNAL ERROR (`pbt`): graph unavoidability lock poisoned");
    // let mut quotient = SCC_QUOTIENT
    //     .lock()
    //     .expect("INTERNAL ERROR (`pbt`): quotient graph lock poisoned");
    // let () = scc::update_unavoidable(
    //     ty,
    //     unavoidable,
    //     &constructors_of,
    //     &mut quotient,
    //     &|variant: &Variant<Erased>| {
    //         const EMPTY: &Multiset<TypeId> = &Multiset::new();
    //         match *variant {
    //             Variant::Algebraic { ref fields } => fields,
    //             Variant::Literal { .. } => EMPTY,
    //         }
    //     },
    // );
    //
    // Arc::clone(
    //     unavoidable
    //         .get(&ty)
    //         .expect("INTERNAL ERROR (`pbt`): missing requested unavoidability result"),
    // )
}
