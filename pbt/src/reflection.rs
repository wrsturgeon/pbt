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
        multiset::Multiset,
        pbt::Pbt,
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::{any::TypeId, iter, mem, num::NonZero},
    std::{collections::hash_map, sync::RwLock},
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
static NAIVE_VARIANTS: RwLock<HashMap<TypeId, Arc<[Constructor<Erased>]>>> = RwLock::new(map());

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

/// A type was not instantiable, e.g. `enum Bad { /* no variants */ }`.
#[derive(Debug)]
#[non_exhaustive]
pub struct Uninstantiable;

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
    pub(crate) fn dedup_fields(&self) -> iter::Copied<hash_map::Keys<'_, TypeId, NonZero<usize>>> {
        self.variant.dedup_fields()
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
    pub(crate) fn dedup_fields(&self) -> iter::Copied<hash_map::Keys<'_, TypeId, NonZero<usize>>> {
        const EMPTY: &HashMap<TypeId, NonZero<usize>> = &map();
        match *self {
            Self::Algebraic { ref field_types } => field_types.iter_dedup(),
            Self::Literal { .. } => EMPTY.keys(),
        }
        .copied()
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
    let () = instantiability::update(ty, &naive, &mut cache, &Constructor::dedup_fields);

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
pub(crate) fn constructors<T>() -> Arc<[Constructor<T>]>
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

/// All variants of a given type,
/// even uninstantiable variants.
#[inline]
#[expect(dead_code, reason = "TODO")]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn naive_variants_of(ty: TypeId) -> Arc<[Constructor<Erased>]> {
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
    variants: &mut HashMap<TypeId, Arc<[Constructor<Erased>]>>,
    visited: &mut HashSet<TypeId>,
) where
    T: Pbt,
{
    // If this type has already been registered, short-circuit:
    let ty = TypeId::of::<T>();
    if variants.contains_key(&ty) || !visited.insert(ty) {
        return;
    }

    // Recurse, i.e. run depth-first search:
    let ordered_naive_variants = T::variants(variants, visited);
    let naive_variants = ordered_naive_variants
        .into_iter()
        .enumerate()
        .map(|(index, variant)| Constructor { index, variant })
        .collect();

    // SAFETY: `T` is only ever the codomain of a function pointer.
    let erased = unsafe {
        mem::transmute::<
            Arc<[Constructor<T>]>, //
            Arc<[Constructor<Erased>]>,
        >(naive_variants)
    };

    let dup: Option<_> = variants.insert(ty, erased);
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
pub(crate) fn register_globally<T>()
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
