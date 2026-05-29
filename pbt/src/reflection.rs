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
    crate::{Pbt, hash::map, instantiability, multiset::Multiset, registration::Registration},
    ahash::HashMap,
    alloc::{collections::BTreeMap, sync::Arc},
    core::{
        any::TypeId,
        cmp,
        hash::{Hash, Hasher},
        iter,
        marker::PhantomData,
        mem,
        num::NonZero,
        ptr,
    },
    std::{collections::hash_map, sync::RwLock},
    wyrand::WyRand,
};

/// Function pointers performing operations on vectors of some type.
static BUCKET_OPS: RwLock<HashMap<TypeId, BucketOps<Erased>>> = RwLock::new(map());

/// All variants of each registered type
/// (transitively through dependencies),
/// *including* uninstantiable variants.
///
/// Graph-theoretically, this is a bipartite graph in which
/// types point to constructors and constructors point to types.
/// Each directed edge means "contains," i.e.
/// "has a field of this type" or "contains this variant."
static NAIVE_VARIANTS: RwLock<BTreeMap<TypeId, Arc<[Constructor<Erased>]>>> =
    RwLock::new(BTreeMap::new());

/// A type's constructors, partitioned into potential leaves and loops,
/// i.e. whether a sub-term of type `Self` is *avoidable* or *reachable*.
#[non_exhaustive]
pub(crate) struct Affordances<SelfType> {
    /// Sorted list of indices of instantiable constructors
    /// for which a sub-term of type `Self` is *avoidable*.
    pub(crate) potential_leaves: Box<[Constructor<SelfType>]>,
    /// Sorted list of indices of instantiable constructors
    /// for which a sub-term of type `Self` is *reachable*.
    pub(crate) potential_loops: Box<[Constructor<SelfType>]>,
}

// TODO: Try once again to use `SelfType` in function types and then
// to transmute the function pointers instead of transmuting internally.
/// Function pointers performing operations on vectors of some type.
#[non_exhaustive]
#[derive(Clone, Copy)]
pub struct BucketOps<SelfType> {
    /// Type-level indicator.
    pub _phantom: PhantomData<SelfType>,
    /// Clone a term of this type.
    pub clone: fn(ptr::NonNull<Erased>) -> ptr::NonNull<Erased>,
    /// Clone a vector of this type.
    pub clone_vec: fn(&Vec<Erased>) -> Vec<Erased>,
    /// Drop a boxed term of this type.
    pub drop: fn(ptr::NonNull<Erased>),
    /// Drop a vector of this type.
    pub drop_vec: fn(Vec<Erased>),
    /// Initialize an empty vector of this type.
    pub empty: fn() -> Vec<Erased>,
    /// Clone an element and push it onto a vector.
    pub push_clone: fn(&mut Vec<Erased>, ptr::NonNull<Erased>),
    /// Remove the `i`th element in O(1) by swapping it with the last element.
    pub swap_remove: fn(&mut Vec<Erased>, usize) -> ptr::NonNull<Erased>,
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
pub(crate) struct Constructor<SelfType: ?Sized> {
    /// The index of this variant under the original source ordering.
    pub(crate) index: usize,
    /// Reflection about this constructor
    /// in terms of its original source-code variant.
    pub(crate) variant: Variant<SelfType>,
}

/// An erased type.
///
/// This type itself is uninstantiable (it's an `enum` without variants):
/// do not use it directly. Instead, `mem::transmute` and be very, very careful.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum Erased {}

/// A deconstruction of a value into its constructor index and its fields.
#[non_exhaustive]
pub struct Parts<F> {
    /// All fields applied to this variant/constructor.
    pub fields: F,
    /// The source-ordering index of the variant used to construct this value.
    pub variant_index: usize,
}

/// All variants of this type.
///
/// If this type is an `enum`, this means exactly what it sounds like.
/// If this type is a `struct`, there's only one variant.
#[non_exhaustive]
pub struct Reflection<SelfType> {
    /// All variants of this type.
    ///
    /// If this type is an `enum`, this means exactly what it sounds like.
    /// If this type is a `struct`, there's only one variant.
    pub variants: Vec<Variant<SelfType>>,
}

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
    pub(crate) const fn is_inductive(&self) -> bool {
        !self.potential_loops.is_empty()
    }
}

impl<T> BucketOps<T> {
    /// Erase type data while maintaining exactly the same function pointers.
    #[inline]
    #[must_use]
    pub const fn erase(self) -> BucketOps<Erased> {
        // SAFETY: Function pointers are the same size no matter the types in these positions.
        unsafe { mem::transmute::<BucketOps<T>, BucketOps<Erased>>(self) }
    }
}

impl<T: Clone> BucketOps<T> {
    /// Derive operations for a statically known type.
    #[inline]
    #[must_use]
    pub const fn derive() -> Self {
        Self {
            clone: |erased_t: ptr::NonNull<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let t: &T = unsafe { erased_t.cast::<T>().as_ref() };
                let cloned: Box<T> = Box::new(t.clone());
                ptr::NonNull::from_mut(Box::leak(cloned)).cast()
            },
            clone_vec: |erased_v: &Vec<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let v: &Vec<T> =
                    unsafe { ptr::from_ref(erased_v).cast::<Vec<T>>().as_ref_unchecked() };
                let cloned: Vec<T> = v.clone();
                // SAFETY: Invariant. Extremely dangerous.
                unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(cloned) }
            },
            drop: |erased_boxed: ptr::NonNull<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let boxed: Box<T> = unsafe { Box::from_raw(erased_boxed.cast::<T>().as_ptr()) };
                let () = drop(boxed);
            },
            drop_vec: |erased_v: Vec<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let typed: Vec<T> = unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(erased_v) };
                let () = drop(typed);
            },
            empty: || {
                // SAFETY: Invariant. Extremely dangerous.
                unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(vec![]) }
            },
            push_clone: |erased_v: &mut Vec<Erased>, erased_t: ptr::NonNull<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let v: &mut Vec<T> =
                    unsafe { ptr::from_mut(erased_v).cast::<Vec<T>>().as_mut_unchecked() };
                // SAFETY: Invariant. Extremely dangerous.
                let t: &T = unsafe { erased_t.cast::<T>().as_ref() };
                v.push(t.clone());
            },
            swap_remove: |erased_v: &mut Vec<Erased>, i: usize| {
                // SAFETY: Invariant. Extremely dangerous.
                let v: &mut Vec<T> =
                    unsafe { ptr::from_mut(erased_v).cast::<Vec<T>>().as_mut_unchecked() };
                let boxed: Box<T> = Box::new(v.swap_remove(i));
                ptr::NonNull::from_mut(Box::leak(boxed)).cast()
            },
            _phantom: PhantomData,
        }
    }
}

impl<T: Clone> Default for BucketOps<T> {
    #[inline]
    fn default() -> Self {
        Self::derive()
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

impl<T> Eq for Constructor<T> {}

impl<T> Hash for Constructor<T> {
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        let () = <usize as Hash>::hash(&self.index, state);
    }
}

impl<T> Ord for Constructor<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        <usize as Ord>::cmp(&self.index, &other.index)
    }
}

impl<T> PartialEq for Constructor<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        <usize as PartialEq>::eq(&self.index, &other.index)
    }
}

impl<T> PartialOrd for Constructor<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(<Self as Ord>::cmp(self, other))
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

/// Function pointers performing operations on vectors of some type.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn bucket_ops_of(ty: TypeId) -> BucketOps<Erased> {
    *BUCKET_OPS
        .read()
        .expect("INTERNAL ERROR (`pbt`): variants lock poisoned")
        .get(&ty)
        .expect("INTERNAL ERROR (`pbt`): unregistered type during bucket-ops lookup")
}

/// Instantiable constructors for each type.
///
/// N.B.: A type's instantiability is as simple as `!constructors.is_empty()`.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
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

/// Register the type `T` and its dependencies
/// in a naive type reflection graph,
/// including any uninstantiable variants.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
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
    let mut bucket_ops = BUCKET_OPS
        .write()
        .expect("INTERNAL ERROR (`pbt`): variants lock poisoned");

    let mut registration = Registration {
        bucket_ops: &mut bucket_ops,
        variants: &mut naive_variants,
    };
    let () = registration.register::<T>();
}
