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
        Pbt, fields::Store, hash::map, instantiability, multiset::Multiset,
        registration::Registration, shrink,
    },
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
/// *including* uninstantiable variants.
///
/// Graph-theoretically, this is a bipartite graph in which
/// types point to constructors and constructors point to types.
/// Each directed edge means "contains," i.e.
/// "has a field of this type" or "contains this variant."
static NAIVE_VARIANTS: RwLock<BTreeMap<TypeId, Constructors<Erased>>> =
    RwLock::new(BTreeMap::new());

// TODO: Try once again to use `SelfType` in function types and then
// to transmute the function pointers instead of transmuting internally.
/// Function pointers performing operations on vectors of some type.
#[non_exhaustive]
#[derive(Clone, Copy)]
pub(crate) struct BucketOps<SelfType> {
    /// Type-level indicator.
    pub(crate) _phantom: PhantomData<SelfType>,
    /// Clone a term of this type.
    pub(crate) clone: fn(ptr::NonNull<Erased>) -> ptr::NonNull<Erased>,
    /// Clone a vector of this type.
    pub(crate) clone_vec: fn(&Vec<Erased>) -> Vec<Erased>,
    /// Deconstruct a boxed value into its constructor index and its fields.
    pub(crate) deconstruct: fn(ptr::NonNull<Erased>) -> Parts<Store>,
    /// Drop a boxed term of this type.
    pub(crate) drop: fn(ptr::NonNull<Erased>),
    /// Drop a vector of this type.
    pub(crate) drop_vec: fn(Vec<Erased>),
    /// Get a *reference* (*not* a `Box`) to the nth element of a vector.
    pub(crate) get: fn(&Vec<Erased>, usize) -> Option<ptr::NonNull<Erased>>,
    /// Pop an element and box it.
    pub(crate) pop: fn(&mut Vec<Erased>) -> Option<ptr::NonNull<Erased>>,
    /// Push a boxed element onto a vector.
    pub(crate) push: fn(&mut Vec<Erased>, ptr::NonNull<Erased>),
    /// Iterate over shrinking candidates for a given initial witness.
    pub(crate) shrink: fn(ptr::NonNull<Erased>) -> Box<dyn Iterator<Item = ptr::NonNull<Erased>>>,
    /// Remove the `i`th element in O(1) by swapping it with the last element.
    pub(crate) swap_remove: fn(&mut Vec<Erased>, usize) -> ptr::NonNull<Erased>,
}

/// Each variant of some type in roughly "smallest-to-largest" order,
/// with ties broken by source ordering.
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
pub(crate) enum Constructors<SelfType: ?Sized> {
    /// The type of each field in this variant.
    /// Order does not matter, but total count does.
    Algebraic(Arc<[Constructor]>),
    /// An opaque function pointer that generates values of this type.
    Literal {
        /// Opaque function pointers that generate values of this type.
        generators: Arc<[fn(&mut WyRand) -> SelfType]>,
        /// An opaque function pointer that shrinks values of this type.
        shrink: fn(SelfType) -> Box<dyn Iterator<Item = SelfType>>,
    },
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
pub(crate) struct Constructor {
    /// The type of each field in this variant.
    /// Order does not matter, but total count does.
    pub(crate) field_types: Multiset<TypeId>,
    /// The index of this variant under the original source ordering.
    pub(crate) index: usize,
}

/// An erased type.
///
/// This type itself is uninstantiable (it's an `enum` without variants):
/// do not use it directly. Instead, `mem::transmute` and be very, very careful.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub(crate) enum Erased {}

/// A deconstruction of a value into its constructor index and its fields.
#[non_exhaustive]
pub struct Parts<F> {
    /// All fields applied to this variant/constructor.
    pub fields: F,
    /// The source-ordering index of the variant used to construct this value.
    pub variant_index: usize,
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
pub struct Variant {
    /// The type of each field in this variant.
    /// Order does not matter, but total count does.
    pub field_types: Multiset<TypeId>,
}

/// Each variant of some type in roughly "smallest-to-largest" order,
/// with ties broken by source ordering.
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
pub enum Variants<SelfType: ?Sized> {
    /// The type of each field in this variant.
    /// Order does not matter, but total count does.
    Algebraic(Vec<Variant>),
    /// An opaque function pointer that generates values of this type.
    Literal {
        /// Opaque function pointers that generate values of this type.
        generators: Vec<fn(&mut WyRand) -> SelfType>,
        /// An opaque function pointer that shrinks values of this type.
        shrink: fn(SelfType) -> Box<dyn Iterator<Item = SelfType>>,
    },
}

impl<T> BucketOps<T> {
    /// Erase type data while maintaining exactly the same function pointers.
    #[inline]
    #[must_use]
    pub(crate) const fn erase(self) -> BucketOps<Erased> {
        // SAFETY: Function pointers are the same size no matter the types in these positions.
        unsafe { mem::transmute::<BucketOps<T>, BucketOps<Erased>>(self) }
    }
}

impl<T: Pbt> BucketOps<T> {
    /// Derive operations for a statically known type.
    #[inline]
    #[must_use]
    pub(crate) const fn derive() -> Self {
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
            deconstruct: |erased_boxed| {
                // SAFETY: Invariant. Extremely dangerous.
                let boxed: Box<T> = unsafe { Box::from_raw(erased_boxed.cast::<T>().as_ptr()) };
                T::deconstruct(*boxed)
            },
            drop: |erased_boxed: ptr::NonNull<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let boxed: Box<T> = unsafe { Box::from_raw(erased_boxed.cast::<T>().as_ptr()) };
                let () = drop(boxed);
            },
            drop_vec: |erased_v: Vec<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let v: Vec<T> = unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(erased_v) };
                let () = drop(v);
            },
            get: |erased_v: &Vec<Erased>, index: usize| {
                // SAFETY: Invariant. Extremely dangerous.
                let v: &Vec<T> =
                    unsafe { ptr::from_ref(erased_v).cast::<Vec<T>>().as_ref_unchecked() };
                Some(ptr::NonNull::from_ref(v.get(index)?).cast::<Erased>())
            },
            pop: |erased_v: &mut Vec<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let v: &mut Vec<T> =
                    unsafe { ptr::from_mut(erased_v).cast::<Vec<T>>().as_mut_unchecked() };
                let boxed = Box::new(v.pop()?);
                Some(ptr::NonNull::from_mut(Box::leak(boxed)).cast())
            },
            push: |erased_v: &mut Vec<Erased>, erased_boxed: ptr::NonNull<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let v: &mut Vec<T> =
                    unsafe { ptr::from_mut(erased_v).cast::<Vec<T>>().as_mut_unchecked() };
                // SAFETY: Invariant. Extremely dangerous.
                let boxed: Box<T> = unsafe { Box::from_raw(erased_boxed.cast::<T>().as_ptr()) };
                let () = v.push(*boxed);
            },
            shrink: |erased_boxed: ptr::NonNull<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let unboxed: T = *unsafe { Box::from_raw(erased_boxed.cast::<T>().as_ptr()) };
                let iter_over_t = shrink::candidates(unboxed);
                let iter_over_erased =
                    iter_over_t.map(|t: T| ptr::NonNull::from_mut(Box::leak(Box::new(t))).cast());
                Box::new(iter_over_erased)
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

impl<T: Pbt> Default for BucketOps<T> {
    #[inline]
    fn default() -> Self {
        Self::derive()
    }
}

impl Constructor {
    /// Iterate over the types of all fields in this variant,
    /// yielding each type exactly once (skipping duplicates).
    #[inline]
    pub(crate) fn dedup_fields(&self) -> iter::Copied<hash_map::Keys<'_, TypeId, NonZero<usize>>> {
        self.field_types.iter_dedup().copied()
    }

    /// The types of all fields in this variant.
    #[inline]
    pub(crate) fn field_types(&self) -> &Multiset<TypeId> {
        &self.field_types
    }
}

impl Clone for Constructor {
    #[inline]
    fn clone(&self) -> Self {
        let Self {
            ref field_types,
            index,
        } = *self;
        Self {
            field_types: field_types.clone(),
            index,
        }
    }
}

impl Eq for Constructor {}

impl Hash for Constructor {
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        let () = <usize as Hash>::hash(&self.index, state);
    }
}

impl Ord for Constructor {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        <usize as Ord>::cmp(&self.index, &other.index)
    }
}

impl PartialEq for Constructor {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        <usize as PartialEq>::eq(&self.index, &other.index)
    }
}

impl PartialOrd for Constructor {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(<Self as Ord>::cmp(self, other))
    }
}

impl<SelfType: ?Sized> Eq for Constructors<SelfType> {}

impl<SelfType: ?Sized> Hash for Constructors<SelfType> {
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        match *self {
            Self::Algebraic(ref ctors) => {
                let () = 0_usize.hash(state);
                let () = ctors.hash(state);
            }
            Self::Literal { ref generators, .. } => {
                let () = 1_usize.hash(state);
                let () = generators.hash(state);
                // The shrinking function is always the same within a given type.
            }
        }
    }
}

impl<SelfType: ?Sized> Ord for Constructors<SelfType> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match (self, other) {
            (
                &Self::Literal {
                    generators: ref lhs,
                    ..
                },
                &Self::Literal {
                    generators: ref rhs,
                    ..
                },
            ) => lhs.cmp(rhs),
            (&Self::Literal { .. }, &Self::Algebraic(_)) => cmp::Ordering::Less,
            (&Self::Algebraic(_), &Self::Literal { .. }) => cmp::Ordering::Greater,
            (&Self::Algebraic(ref lhs), &Self::Algebraic(ref rhs)) => lhs.cmp(rhs),
        }
    }
}

impl<SelfType: ?Sized> PartialEq for Constructors<SelfType> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                &Self::Literal {
                    generators: ref lhs,
                    ..
                },
                &Self::Literal {
                    generators: ref rhs,
                    ..
                },
            ) => lhs.eq(rhs),
            (&Self::Algebraic(ref lhs), &Self::Algebraic(ref rhs)) => lhs.eq(rhs),
            _ => false,
        }
    }
}

impl<SelfType: ?Sized> PartialOrd for Constructors<SelfType> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(<Self as Ord>::cmp(self, other))
    }
}

impl<SelfType: ?Sized> Clone for Constructors<SelfType> {
    #[inline]
    fn clone(&self) -> Self {
        match *self {
            Self::Algebraic(ref constructors) => Self::Algebraic(Arc::clone(constructors)),
            Self::Literal {
                ref generators,
                shrink,
            } => Self::Literal {
                generators: Arc::clone(generators),
                shrink,
            },
        }
    }
}

impl<SelfType: ?Sized> Constructors<SelfType> {
    /// Algebraic constructors exposed as a slice.
    #[inline]
    #[must_use]
    pub(crate) fn algebraic(&self) -> &[Constructor] {
        match *self {
            Self::Algebraic(ref constructors) => constructors,
            Self::Literal { .. } => &[],
        }
    }

    /// Whether this type has no productive variants.
    #[inline]
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        match *self {
            Self::Algebraic(ref constructors) => constructors.is_empty(),
            Self::Literal { ref generators, .. } => generators.is_empty(),
        }
    }
}

impl<SelfType> Variants<SelfType> {
    /// Erase type data while maintaining exactly the same function pointers.
    #[inline]
    #[must_use]
    pub(crate) fn erase(self) -> Constructors<Erased> {
        match self {
            Self::Algebraic(constructors) => Constructors::Algebraic(
                constructors
                    .into_iter()
                    .enumerate()
                    .map(|(index, Variant { field_types })| Constructor { field_types, index })
                    .collect(),
            ),
            Self::Literal { generators, shrink } => {
                let erased_generators: Arc<[fn(&mut WyRand) -> Erased]> =
                    generators
                        .into_iter()
                        .map(|f| {
                            // SAFETY: Function pointers are the same size no matter the types in these positions.
                            unsafe {
                                mem::transmute::<
                                    fn(&mut WyRand) -> SelfType,
                                    fn(&mut WyRand) -> Erased,
                                >(f)
                            }
                        })
                        .collect();
                // SAFETY: Function pointers are the same size no matter the types in these positions.
                let shrink = unsafe {
                    mem::transmute::<
                        fn(SelfType) -> Box<dyn Iterator<Item = SelfType>>,
                        fn(Erased) -> Box<dyn Iterator<Item = Erased>>,
                    >(shrink)
                };
                Constructors::Literal {
                    generators: erased_generators,
                    shrink,
                }
            }
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

/// Whether a registered type is represented by opaque literal operations.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn is_literal(ty: TypeId) -> bool {
    matches!(
        NAIVE_VARIANTS
            .read()
            .expect("INTERNAL ERROR (`pbt`): variants lock poisoned")
            .get(&ty),
        Some(Constructors::Literal { .. })
    )
}

/// Shrink a literal value using its type-level literal shrinker.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn shrink_literal<T>(t: T) -> Option<Box<dyn Iterator<Item = T>>>
where
    T: 'static,
{
    let ty = TypeId::of::<T>();
    let shrink = {
        let naive_variants = NAIVE_VARIANTS
            .read()
            .expect("INTERNAL ERROR (`pbt`): variants lock poisoned");
        let Constructors::Literal { shrink, .. } = *naive_variants.get(&ty)? else {
            return None;
        };
        shrink
    };

    // SAFETY: `Registration::register::<T>` erased this function pointer.
    let shrink = unsafe {
        mem::transmute::<
            fn(Erased) -> Box<dyn Iterator<Item = Erased>>,
            fn(T) -> Box<dyn Iterator<Item = T>>,
        >(shrink)
    };
    Some(shrink(t))
}

/// Instantiable constructors for each type.
///
/// N.B.: A type's instantiability is as simple as `!constructors.is_empty()`.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn constructors_of(ty: TypeId) -> Constructors<Erased> {
    static CACHE: RwLock<HashMap<TypeId, Constructors<Erased>>> = RwLock::new(map());

    if let Some(cached) = CACHE
        .read()
        .expect("INTERNAL ERROR (`pbt`): instantiability lock poisoned")
        .get(&ty)
    {
        return cached.clone();
    }

    let naive = NAIVE_VARIANTS
        .read()
        .expect("INTERNAL ERROR (`pbt`): variants lock poisoned");
    let mut cache = CACHE
        .write()
        .expect("INTERNAL ERROR (`pbt`): instantiability lock poisoned");
    let () = instantiability::update(ty, &naive, &mut cache, &Constructor::field_types);

    cache
        .get(&ty)
        .expect("INTERNAL ERROR (`pbt`): unregistered type during instantiability analysis")
        .clone()
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
