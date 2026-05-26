//! Graph-theoretic analysis of the structure of algebraic data types.
//!
//! Rust types are translated s.t. a `struct` is a singleton degenerate case (one variant).

use {
    crate::{
        hash::{map, set},
        multiset::Multiset,
        pbt::Pbt,
        scc,
        union_find::{RootElement, UnionFind},
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::{
        any::{self, TypeId},
        iter, mem,
    },
    std::sync::{Mutex, RwLock},
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

/// Instantiable constructors for each type.
///
/// This is the least fixed point of the following relation:
///
/// - Constructors are instantiable if all their fields' types are instantiable.
/// - Types are instantiable if any of their constructors are instantiable.
///
/// We specify the *least* fixed point to exclude
/// coinductive/infinitely-sized types like `struct Loop(Box<Self>)`.
///
/// A nice consequence is that a type's instantiability *after* this pass
/// is merely `!constructors.is_empty()`.
#[expect(dead_code, reason = "TODO")]
static CONSTRUCTORS: RwLock<HashMap<TypeId, Arc<[Variant<Erased>]>>> = RwLock::new(map());

/// Every registered type and its directed edges (representing "has a field of this type"),
/// without any non-local graph-theoretic information.
///
/// This graph may be cyclic (and it often will be, exactly when there exists an inductive type),
/// but incoming edges will never affect inductive logic unless it closes a loop,
/// so graph-theoretic analyses are free to care only about vertices reachable from some type.
///
/// The above realization also leads to the following insight:
/// *strongly connected components define mutually inductive types.*
static VERTICES: RwLock<HashMap<TypeId, Arc<TypeGraphVertex>>> = RwLock::new(map());

/// DFS of reachability between *strongly connected components* of the type-dependency graph,
/// i.e. the graph in which vertices are types and edges represent "has a field of this type."
///
/// The key insight is that *strongly connected components define mutually inductive types.*
///
/// To test reachability between *types* (not SCCs themselves),
/// first lift each type to its enclosing SCC, then test SCC reachability.
/// This is especially nice since this quotient graph is a DAG by construction,
/// so this reachability logic can safely assume the absence of cycles.
static QUOTIENT: Mutex<UnionFind<TypeId, Arc<scc::QuotientVertex<TypeId>>>> =
    Mutex::new(UnionFind::new());

/// Transitive reachability between *strongly connected components* of the type-dependency graph,
/// i.e. the graph in which vertices are types and edges represent "has a field of this type."
///
/// The key insight is that *strongly connected components define mutually inductive types.*
///
/// This is useful when determining whether some variant is potentially inductive:
/// if any of its fields can reach `Self`, then it can potentially be inductive,
/// so we should consider it when we have plenty of size left to generate.
static REACHABLE: RwLock<HashMap<RootElement<TypeId>, Arc<HashSet<RootElement<TypeId>>>>> =
    RwLock::new(map());

/// This encodes the notion that "if we start generating a term of type A,
/// then must unavoidably generate terms of type B, C, ... in the process."
///
/// This is useful when determining whether some variant could be a leaf:
/// if any of its fields unavoidably reach `Self`, then it can never be a leaf,
/// so we should avoid it when we're running out of size remaining to generate.
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
#[expect(
    clippy::expect_used,
    clippy::missing_panics_doc,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn reachable(scc_root: RootElement<TypeId>) -> Arc<HashSet<RootElement<TypeId>>> {
    scc::reachable(
        &mut REACHABLE
            .write()
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
/// N.B.: This function locks `UNAVOIDABLE`, `VERTICES`, and `QUOTIENT` in that order.
#[inline]
#[expect(
    clippy::expect_used,
    clippy::missing_panics_doc,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn unavoidable(ty: TypeId) -> Arc<HashSet<TypeId>> {
    let unavoidable = &mut UNAVOIDABLE
        .write()
        .expect("INTERNAL ERROR (`pbt`): graph unavoidability lock poisoned");
    let vertices = VERTICES
        .read()
        .expect("INTERNAL ERROR (`pbt`): reflection registry lock poisoned");
    let mut quotient = QUOTIENT
        .lock()
        .expect("INTERNAL ERROR (`pbt`): quotient graph lock poisoned");
    let () = scc::update_unavoidable(
        ty,
        unavoidable,
        &vertices,
        &mut quotient,
        &|adt: &Arc<TypeGraphVertex>| &*adt.reflection.variants,
        &|variant: &Variant<Erased>| {
            const EMPTY: &Multiset<TypeId> = &Multiset::new();
            match *variant {
                Variant::Algebraic { ref fields } => fields,
                Variant::Literal { .. } => EMPTY,
            }
        },
    );

    Arc::clone(
        unavoidable
            .get(&ty)
            .expect("INTERNAL ERROR (`pbt`): missing requested unavoidability result"),
    )
}

/// Register the type `T` (and all of its dependencies)
/// in the global type reflection graph.
///
/// This prunes all uninstantiable variants.
///
/// # Panics
///
/// If a `Pbt` implementation fails to register one of its field dependencies.
#[inline]
#[expect(clippy::implicit_hasher, reason = "all in on `ahash`")]
#[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn register_instantiable<T>(vertices: &mut HashMap<TypeId, Arc<[Variant<Erased>]>>)
where
    T: Pbt,
{
    if vertices.contains_key(&TypeId::of::<T>()) {
        return;
    }

    // Collect the *naive* graph (including any uninstantiable variants):
    let mut naive_reachable = map();
    let mut visited = set();
    let () = register::<T>(&mut naive_reachable, &mut visited);

    let mut type_masks: HashMap<TypeId, bool> = map();
    let mut variant_masks: HashMap<TypeId, Box<[bool]>> = map();
    for (&ty, variants) in &mut naive_reachable {
        if let Some(already_registered) = vertices.get(&ty) {
            *variants = Arc::clone(already_registered);
            let _: Option<_> = type_masks.insert(ty, !variants.is_empty());
            let _: Option<_> =
                variant_masks.insert(ty, iter::repeat_n(true, variants.len()).collect());
        } else {
            let _: Option<_> = type_masks.insert(ty, false);
            let _: Option<_> =
                variant_masks.insert(ty, iter::repeat_n(false, variants.len()).collect());
        }
    }

    'fixed_point: loop {
        let mut changed = false;

        'types: for (&ty, variants) in &naive_reachable {
            let variant_mask: &mut [bool] = variant_masks.get_mut(&ty).expect(
                "INTERNAL ERROR (`pbt`): unregistered type during instantiability analysis",
            );
            debug_assert_eq!(
                variant_mask.len(),
                variants.len(),
                "INTERNAL ERROR (`pbt`): variant size mismatch during instantiability analysis",
            );
            'variants: for i in 0..variants.len() {
                // SAFETY: Loop bounds above.
                let variant = unsafe { variants.get_unchecked(i) };
                // SAFETY: Checked above (see the `debug_assert_eq!`).
                let mask = unsafe { variant_mask.get_unchecked_mut(i) };
                if *mask {
                    continue 'variants;
                }
                let instantiable = match *variant {
                    Variant::Algebraic { ref fields } => fields.iter_dedup().all(|&t| {
                        *type_masks.get(&t).expect(
                            "INTERNAL ERROR (`pbt`): unregistered type during instantiability analysis",
                        )
                    }),
                    Variant::Literal { .. } => true,
                };
                if instantiable {
                    *mask = true;
                    changed = true;
                }
            }
            let mask: &mut bool = type_masks.get_mut(&ty).expect(
                "INTERNAL ERROR (`pbt`): unregistered type during instantiability analysis",
            );
            if *mask {
                continue 'types;
            }
            if variant_mask.iter().any(|&b| b) {
                *mask = true;
                changed = true;
            }
        }

        if !changed {
            break 'fixed_point;
        }
    }

    for (ty, variants) in naive_reachable {
        let _: &mut _ = vertices
            .entry(ty)
            .or_insert_with(|| -> Arc<[Variant<Erased>]> {
                let variant_mask: &[bool] = variant_masks.get(&ty).expect(
                    "INTERNAL ERROR (`pbt`): unregistered type during instantiability analysis",
                );
                debug_assert_eq!(
                    variant_mask.len(),
                    variants.len(),
                    "INTERNAL ERROR (`pbt`): variant size mismatch during instantiability analysis",
                );
                variants
                    .iter()
                    .zip(variant_mask)
                    .filter_map(|(variant, &enabled)| enabled.then_some(variant))
                    .cloned()
                    .collect()
            });
    }
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
    let variants = T::variants(vertices, visited);

    // SAFETY: `T` is only ever the codomain of a function pointer.
    let erased = unsafe {
        mem::transmute::<
            Arc<[Variant<T>]>, //
            Arc<[Variant<Erased>]>,
        >(variants)
    };

    // Insert the recursive result:
    let dup: Option<_> = vertices.insert(ty, erased);
    assert!(dup.is_none(), "INTERNAL ERROR (`pbt`): TOCTOU during DFS");
}
