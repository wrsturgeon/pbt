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
        any::{TypeId, type_name},
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
    /// Deserialize JSON into a vector of this type.
    pub(crate) deserialize: fn(&Vec<serde_json::Value>) -> Option<Vec<Erased>>,
    /// Drop a boxed term of this type.
    pub(crate) drop: fn(ptr::NonNull<Erased>),
    /// Drop a vector of this type.
    pub(crate) drop_vec: fn(Vec<Erased>),
    /// Create an empty vector of this type.
    pub(crate) empty: fn() -> Vec<Erased>,
    /// Get a *reference* (*not* a `Box`) to the nth element of a vector.
    pub(crate) get: fn(&Vec<Erased>, usize) -> Option<ptr::NonNull<Erased>>,
    /// The name of this type.
    pub(crate) name: fn() -> &'static str,
    /// Pop an element and box it.
    pub(crate) pop: fn(&mut Vec<Erased>) -> Option<ptr::NonNull<Erased>>,
    /// Push a boxed element onto a vector.
    pub(crate) push: fn(&mut Vec<Erased>, ptr::NonNull<Erased>),
    /// Serialize a vector of this type into JSON.
    pub(crate) serialize: fn(Vec<Erased>) -> Vec<serde_json::Value>,
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
pub(crate) enum Constructors<SelfType> {
    /// The type of each field in this variant.
    /// Order does not matter, but total count does.
    Algebraic(Arc<[Constructor]>),
    /// An opaque function pointer that generates values of this type.
    Literal {
        /// Deserialize JSON into this type.
        deserialize: fn(&serde_json::Value) -> Option<SelfType>,
        /// Opaque function pointers that generate values of this type.
        generators: Arc<[fn(&mut WyRand) -> SelfType]>,
        /// Serialize this type into JSON.
        serialize: fn(&SelfType) -> serde_json::Value,
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
    /// The 1-indexed position of this variant under the original source ordering.
    pub(crate) index: NonZero<usize>,
}

/// An erased type.
///
/// This type itself is uninstantiable (it's an `enum` without variants):
/// do not use it directly. Instead, `mem::transmute` and be very, very careful.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub(crate) enum Erased {}

/// A deconstruction of a value into its constructor index and its fields.
#[expect(
    clippy::exhaustive_structs,
    reason = "`derive(Pbt)` must construct and destructure this across crate boundaries"
)]
pub struct Parts<F> {
    /// All fields applied to this variant/constructor.
    pub fields: F,
    /// The 1-indexed source-ordering position of the variant used to construct this value.
    pub variant_index: Option<NonZero<usize>>,
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
#[expect(
    clippy::exhaustive_structs,
    reason = "`derive(Pbt)` must construct this across crate boundaries"
)]
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
pub enum Variants<SelfType> {
    /// The type of each field in this variant.
    /// Order does not matter, but total count does.
    Algebraic(Vec<Variant>),
    /// An opaque function pointer that generates values of this type.
    Literal {
        // TODO: automatically enumerate corner cases
        /// Deserialize JSON into this type.
        deserialize: fn(&serde_json::Value) -> Option<SelfType>,
        /// Opaque function pointers that generate values of this type.
        generators: Vec<fn(&mut WyRand) -> SelfType>,
        /// Serialize this type into JSON.
        serialize: fn(&SelfType) -> serde_json::Value,
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
            deserialize: |jsons: &Vec<serde_json::Value>| {
                let v: Vec<T> = jsons
                    .iter()
                    .map(Parts::deserialize)
                    .collect::<Option<Vec<T>>>()?;
                // SAFETY: Invariant. Extremely dangerous.
                Some(unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(v) })
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
            empty: || {
                let v: Vec<T> = Vec::new();
                // SAFETY: Invariant. Extremely dangerous.
                unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(v) }
            },
            get: |erased_v: &Vec<Erased>, index: usize| {
                // SAFETY: Invariant. Extremely dangerous.
                let v: &Vec<T> =
                    unsafe { ptr::from_ref(erased_v).cast::<Vec<T>>().as_ref_unchecked() };
                Some(ptr::NonNull::from_ref(v.get(index)?).cast::<Erased>())
            },
            name: type_name::<T>,
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
            serialize: |erased_v: Vec<Erased>| {
                // SAFETY: Invariant. Extremely dangerous.
                let v: Vec<T> = unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(erased_v) };
                if let Constructors::Literal { serialize, .. } = constructors_of(TypeId::of::<T>())
                {
                    // SAFETY: Invariant. Extremely dangerous.
                    let serialize_typed = unsafe {
                        mem::transmute::<
                            fn(&Erased) -> serde_json::Value,
                            fn(&T) -> serde_json::Value,
                        >(serialize)
                    };
                    v.iter().map(serialize_typed).collect()
                } else {
                    v.into_iter()
                        .map(|t: T| t.deconstruct().serialize())
                        .collect()
                }
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
        let () = <NonZero<usize> as Hash>::hash(&self.index, state);
    }
}

impl Ord for Constructor {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        <NonZero<usize> as Ord>::cmp(&self.index, &other.index)
    }
}

impl PartialEq for Constructor {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        <NonZero<usize> as PartialEq>::eq(&self.index, &other.index)
    }
}

impl PartialOrd for Constructor {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(<Self as Ord>::cmp(self, other))
    }
}

impl<SelfType> Eq for Constructors<SelfType> {}

impl<SelfType> Hash for Constructors<SelfType> {
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

impl<SelfType> Ord for Constructors<SelfType> {
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

impl<SelfType> PartialEq for Constructors<SelfType> {
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

impl<SelfType> PartialOrd for Constructors<SelfType> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(<Self as Ord>::cmp(self, other))
    }
}

impl<SelfType> Clone for Constructors<SelfType> {
    #[inline]
    fn clone(&self) -> Self {
        match *self {
            Self::Algebraic(ref constructors) => Self::Algebraic(Arc::clone(constructors)),
            Self::Literal {
                deserialize,
                ref generators,
                serialize,
                shrink,
            } => Self::Literal {
                deserialize,
                generators: Arc::clone(generators),
                serialize,
                shrink,
            },
        }
    }
}

impl<SelfType> Constructors<SelfType> {
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

impl Parts<Store> {
    /// Deserialize from JSON.
    #[inline]
    #[expect(
        clippy::expect_used,
        clippy::unwrap_in_result,
        reason = "Internal invariants: violations should fail loudly."
    )]
    pub(crate) fn deserialize<T>(json: &serde_json::Value) -> Option<T>
    where
        T: Pbt,
    {
        let ty = TypeId::of::<T>();
        let ctors = match constructors_of(ty) {
            Constructors::Algebraic(ref arc_ctors) => Arc::clone(arc_ctors),
            Constructors::Literal { deserialize, .. } => {
                // SAFETY: Invariant. Extremely dangerous.
                let deserialize_typed = unsafe {
                    mem::transmute::<
                        fn(&serde_json::Value) -> Option<Erased>,
                        fn(&serde_json::Value) -> Option<T>,
                    >(deserialize)
                };
                return deserialize_typed(json);
            }
        };

        let serde_json::Value::Object(ref map) = *json else {
            return None;
        };
        let variant_index: Option<NonZero<usize>> =
            map.get("index").and_then(|json_index: &serde_json::Value| {
                let serde_json::Value::String(ref s) = *json_index else {
                    return None;
                };
                NonZero::new(s.parse().ok()?)
            });

        let algebraic_variant_index = variant_index?;
        let ctor = ctors
            .iter()
            // TODO: store naive `Variant`s (without indices) to eliminate the below linear search
            .find(|&ctor| ctor.index == algebraic_variant_index)
            .expect("INTERNAL ERROR (`pbt`): deserializing non-existent constructor");

        let fields = Store::deserialize(map.get("fields")?, &ctor.field_types)?;
        Some(T::construct(Self {
            fields,
            variant_index,
        }))
    }

    /// Serialize into JSON.
    #[inline]
    #[expect(
        clippy::expect_used,
        clippy::panic,
        reason = "Internal invariants: violations should fail loudly."
    )]
    pub(crate) fn serialize(self) -> serde_json::Value {
        let Self {
            mut fields,
            variant_index,
        } = self;
        if let Some(index) = variant_index {
            serde_json::Value::Object(
                [
                    ("fields".to_owned(), fields.serialize()),
                    ("index".to_owned(), index.to_string().into()),
                ]
                .into_iter()
                .collect(),
            )
        } else {
            let (ty, erased) = fields
                .pop_erased()
                .expect("INTERNAL ERROR (`pbt`): serializing a non-existent literal");
            debug_assert!(
                fields.pop_erased().is_none(),
                "INTERNAL ERROR (`pbt`): serializing a literal that contains multitudes",
            );
            let ctors = constructors_of(ty);
            let Constructors::Literal { serialize, .. } = ctors else {
                panic!("INTERNAL ERROR (`pbt`): serializing a literal that think it's algebraic");
            };
            // SAFETY: References are non-null pointers with extra type-level info.
            let serialize_ptr = unsafe {
                mem::transmute::<
                    fn(&Erased) -> serde_json::Value,
                    fn(ptr::NonNull<Erased>) -> serde_json::Value,
                >(serialize)
            };
            let json = serialize_ptr(erased);
            let () = (bucket_ops_of(ty).drop)(erased);
            json
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
                    .map(|(zero_indexed, Variant { field_types })| Constructor {
                        field_types,
                        #[expect(
                            clippy::arithmetic_side_effects,
                            reason = "If an index is `usize::MAX`, there are bigger issues."
                        )]
                        index: {
                            // SAFETY: If an index is `usize::MAX`, there are bigger issues,
                            // so this should panic. Otherwise, the result will be nonzero.
                            unsafe { NonZero::new_unchecked(zero_indexed + 1) }
                        },
                    })
                    .collect(),
            ),
            Self::Literal {
                deserialize,
                generators,
                serialize,
                shrink,
            } => {
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
                let erased_deserialize = unsafe {
                    mem::transmute::<
                        fn(&serde_json::Value) -> Option<SelfType>,
                        fn(&serde_json::Value) -> Option<Erased>,
                    >(deserialize)
                };
                // SAFETY: Function pointers are the same size no matter the types in these positions.
                let erased_serialize = unsafe {
                    mem::transmute::<
                        fn(&SelfType) -> serde_json::Value,
                        fn(&Erased) -> serde_json::Value,
                    >(serialize)
                };
                // SAFETY: Function pointers are the same size no matter the types in these positions.
                let erased_shrink = unsafe {
                    mem::transmute::<
                        fn(SelfType) -> Box<dyn Iterator<Item = SelfType>>,
                        fn(Erased) -> Box<dyn Iterator<Item = Erased>>,
                    >(shrink)
                };
                Constructors::Literal {
                    deserialize: erased_deserialize,
                    generators: erased_generators,
                    serialize: erased_serialize,
                    shrink: erased_shrink,
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
    let () = instantiability::update(ty, &naive, &mut cache);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(
        clippy::todo,
        reason = "the correct behavior is never to reach these `todo`s"
    )]
    fn literal_constructors_eq_iff_generators_eq() {
        let f: for<'a> fn(&'a mut _) -> _ = |_| todo!();
        let g: for<'a> fn(&'a mut _) -> _ = |_| todo!();

        let ctors_f = Constructors::Literal {
            deserialize: |_| todo!(),
            generators: Arc::new([f]),
            serialize: |_| todo!(),
            shrink: |_| todo!(),
        };
        let ctors_g = Constructors::Literal {
            deserialize: |_| todo!(),
            generators: Arc::new([g]),
            serialize: |_| todo!(),
            shrink: |_| todo!(),
        };

        assert_eq!(ctors_f, ctors_f);
        assert_eq!(ctors_g, ctors_g);
        assert_ne!(ctors_f, ctors_g);
        assert_ne!(ctors_g, ctors_f);
    }
}
