use {
    crate::{
        SEED,
        cache::{self, CachedTerm},
        multiset::Multiset,
        pbt::{
            Algebraic, CtorFn, ElimFn, IndexedCtorFn, IntroductionRule, Literal,
            MaybeUninstantiable, Pbt, TypeFormer, deserialize_cached_term_into_buckets,
        },
        scc::{self, StronglyConnectedComponents},
        shrink::shrink,
        size::Sizes,
    },
    ahash::RandomState,
    core::{
        any::{TypeId, type_name},
        fmt, iter, mem,
        num::NonZero,
        ptr,
    },
    std::{
        collections::{BTreeMap, BTreeSet, btree_map},
        sync::{Arc, LazyLock, OnceLock, PoisonError, RwLock},
    },
    wyrand::WyRand,
};

/// One, as a non-zero integer. Stupid but efficient.
const ONE: NonZero<usize> = NonZero::new(1).unwrap();

/// A statically unknown type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum Erased {
    // uninstantiable
}

/// The erased vtable used to manipulate one concrete bucket type inside
/// [`ErasedTermBucket`] values without carrying monomorphized closures inline.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct ErasedBucketOps {
    pub clone: for<'t> fn(&'t Vec<Erased>) -> Vec<Erased>,
    pub debug: for<'t, 'm, 'f> fn(&'t Vec<Erased>, &'m mut fmt::Formatter<'f>) -> fmt::Result,
    pub drop: fn(Vec<Erased>),
    pub eq: for<'lhs, 'rhs> fn(&'lhs Vec<Erased>, &'rhs Vec<Erased>) -> bool,
    pub pop_serialize: fn(&mut Vec<Erased>) -> Option<CachedTerm>,
    pub shrink: fn(Vec<Erased>) -> Box<dyn Iterator<Item = Vec<Erased>>>,
}

/// An erased term bucket together with the type key needed to recover its vtable.
#[non_exhaustive]
pub struct ErasedTermBucket {
    pub terms: Vec<Erased>,
    pub ty: Type,
}

/// A map from types to ordered collections of terms of those types.
/// This is used e.g. for constructors:
/// each constructor knows the multiset of types it needs to fill its fields,
/// so it can request exactly enough terms of various types to do so.
#[non_exhaustive]
#[repr(transparent)]
pub struct ErasedTermBuckets {
    /// A map from types to ordered collections of terms of those types.
    pub map: BTreeMap<Type, ErasedTermBucket>,
}

/// Backward-compatible alias for [`ErasedBucketOps`].
pub type BucketOps = ErasedBucketOps;

/// Backward-compatible alias for [`ErasedTermBucket`].
pub type Terms = ErasedTermBucket;

/// Backward-compatible alias for [`ErasedTermBuckets`].
pub type TermsOfVariousTypes = ErasedTermBuckets;

#[non_exhaustive]
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Type(TypeId);

/// A vertex in the type-dependency graph,
/// indexed by its opaque Rust `TypeId`,
/// whose outgoing edges are determined by the
/// notion of this type "containing" another type,
/// i.e. containing some (variant with) some field of that type.
/// We distinguish the sets of types that *may* or *must*
/// be contained in any term of this type;
/// the former is "reachable" and the latter is "unavoidable."
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Vertex {
    /// Whether `Self` is inductive.
    /// Internally, this asks whether its
    /// strongly connected component is nontrivial.
    pub cached_inductivity: OnceLock<bool>,
    /// The opaque Rust ID for this type.
    pub ty: Type,
    /// The minimal bag of types that *must* be contained in any term of this type.
    /// This field is not a multiset because, if this type is inductive,
    /// then the logic around how many times each type is unavoidable
    /// is too complex to be worth doing, especially since it provides no runtime benefit.
    pub unavoidable: BTreeSet<Type>,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct CtorVertex {
    /// Information about this particular constructor,
    /// not about the type as a whole.
    pub constructor: CtorInfo,
    /// Graph-theoretic information about the constructor
    /// as if it were a single `struct` type.
    pub vertex: Vertex,
}

#[non_exhaustive]
/// Fully computed reflection metadata for one concrete type.
#[derive(Debug)]
pub struct TypeInfo {
    /// If this is a "big" type:
    /// either inductive or contains a big type.
    pub cached_big: OnceLock<bool>,
    /// Decode one cached term of this type and push it into erased term buckets.
    pub deserialize_cached_term_into_buckets: fn(&CachedTerm, &mut ErasedTermBuckets) -> bool,
    /// Erased bucket operations for values of this concrete type.
    pub erased_bucket_ops: ErasedBucketOps,
    /// The pretty-printed name of this type.
    pub name: &'static str,
    /// Whether this type is uninteresting: specifically, whether it is either
    /// non-inductive or a trivial wrapper around exactly one (other) type.
    /// Note that uninstantiable types *are* interesting, i.e. nontrivial.
    pub trivial: bool,
    pub type_former: PrecomputedTypeFormer,
    /// The union and intersection of the bag of types that
    /// may be contained in a value of this type.
    pub vertex: Vertex,
}

/// The fully computed registration payload for one type before it is published
/// to the global registry and SCC graph.
struct ComputedTypeRegistration {
    /// The registry payload to publish for this type.
    info: TypeInfo,
    /// The SCC graph node to publish alongside the registry payload.
    scc_node: scc::Node,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct CtorInfo {
    /// Generate precisely enough arbitrary fields
    /// to immediately invoke this constructor.
    pub arbitrary_fields:
        for<'prng> fn(&'prng mut WyRand, Sizes) -> Result<TermsOfVariousTypes, MaybeUninstantiable>,
    /// The number of "big" types: types that either
    /// are inductive themselves or contain a big type.
    pub cached_n_big: OnceLock<usize>,
    /// The multiset of types necessary to call this constructor.
    pub immediate: Multiset<Type>,
    /// 1-indexed constructor/variant index.
    pub index: NonZero<usize>,
}

#[non_exhaustive]
#[derive(Debug)]
pub struct AlgebraicTypeFormer {
    /// The exhaustive disjoint set of methods
    /// to pbt a term of this type,
    /// each tagged with information about its type-level properties.
    pub all_constructors: Vec<(CtorFn<Erased>, CtorVertex)>,
    /// All constructors for which `Self` is *unreachable*.
    /// Use this (when non-empty) to *force* generation of
    /// a *strictly smaller* value (in some sense).
    pub cached_guaranteed_leaves: OnceLock<Vec<IndexedCtorFn<Erased>>>,
    /// All constructors for which `Self` is *unavoidable*.
    /// Use this (when non-empty) to *force* generation of
    /// a *strictly larger* value (in some sense).
    pub cached_guaranteed_loops: OnceLock<Vec<IndexedCtorFn<Erased>>>,
    /// All constructors for which `Self` is *avoidable*.
    /// This is guaranteed to be non-empty because
    /// Rust disallows coinductive types (i.e. streams, infinite-size types, etc.)
    /// Use this (when non-empty) to *allow* generation of
    /// a smaller value (in some sense).
    pub cached_potential_leaves: OnceLock<Vec<IndexedCtorFn<Erased>>>,
    /// All constructors for which `Self` is *reachable*.
    /// Use this (when non-empty) to *allow* generation of
    /// a *larger* value (in some sense).
    pub cached_potential_loops: OnceLock<Vec<IndexedCtorFn<Erased>>>,
    /// Decompose this value into a
    /// constructor (by index) and
    /// its associated fields.
    pub eliminator: ElimFn<Erased>,
}

#[non_exhaustive]
#[derive(Debug)]
pub enum PrecomputedTypeFormer {
    Algebraic(AlgebraicTypeFormer),
    Literal {
        deserialize: fn(&str) -> Option<Erased>,
        generate: for<'prng> fn(&'prng mut WyRand) -> Erased,
        serialize: for<'t> fn(&'t Erased) -> String,
        shrink: fn(Erased) -> Box<dyn Iterator<Item = Erased>>,
    },
}

impl AlgebraicTypeFormer {
    /// All constructors for which `Self` is *unreachable*.
    /// Use this (when non-empty) to *force* generation of
    /// a *strictly smaller* value (in some sense).
    #[inline]
    #[must_use]
    pub fn guaranteed_leaves(&self) -> &[IndexedCtorFn<Erased>] {
        self.cached_guaranteed_leaves.get_or_init(|| {
            self.all_constructors
                .iter()
                .filter(|&&(_, ref c)| c.is_guaranteed_leaf())
                .map(|&(call, ref c)| IndexedCtorFn {
                    arbitrary_fields: c.constructor.arbitrary_fields,
                    call,
                    index: c.constructor.index,
                    n_big: c.constructor.n_big(),
                })
                .collect()
        })
    }

    /// All constructors for which `Self` is *unavoidable*.
    /// Use this (when non-empty) to *force* generation of
    /// a *strictly larger* value (in some sense).
    #[inline]
    #[must_use]
    pub fn guaranteed_loops(&self) -> &[IndexedCtorFn<Erased>] {
        self.cached_guaranteed_loops.get_or_init(|| {
            self.all_constructors
                .iter()
                .filter(|&&(_, ref c)| c.is_guaranteed_loop())
                .map(|&(call, ref c)| IndexedCtorFn {
                    arbitrary_fields: c.constructor.arbitrary_fields,
                    call,
                    index: c.constructor.index,
                    n_big: c.constructor.n_big(),
                })
                .collect()
        })
    }

    /// Partition a set of constructors into subsets
    /// that will be useful for generation and shrinking.
    /// # Panics
    /// If constructors are out of order (for bookkeeping)
    /// or if every constructor forces creation of
    /// another term of type `Self` (since generation would never halt).
    #[inline]
    #[must_use]
    pub fn new<T>(all_constructors: Vec<(CtorFn<T>, CtorVertex)>, eliminator: ElimFn<T>) -> Self {
        // SAFETY: Same size, still a function pointer with the same arguments.
        let all_constructors = unsafe {
            mem::transmute::<Vec<(CtorFn<T>, CtorVertex)>, Vec<(CtorFn<Erased>, CtorVertex)>>(
                all_constructors,
            )
        };
        #[cfg(debug_assertions)]
        {
            let ctor_indices = all_constructors
                .iter()
                .map(|&(_, ref cv)| cv.constructor.index);
            // SAFETY: Starts from one, monotonically increasing, ergo never zero
            let expected_indices =
                (1..=all_constructors.len()).map(|i| unsafe { NonZero::new_unchecked(i) });
            assert!(
                Iterator::eq(ctor_indices, expected_indices),
                "Constructor indices are out of order (should be 1, 2, ...): {all_constructors:#?}",
            );
        }
        Self {
            all_constructors,
            // SAFETY: Never used in its erased form, and
            // `Vec<_>`s all have the same size+alignment.
            eliminator: unsafe { mem::transmute::<ElimFn<T>, ElimFn<Erased>>(eliminator) },
            cached_guaranteed_leaves: OnceLock::new(),
            cached_guaranteed_loops: OnceLock::new(),
            cached_potential_leaves: OnceLock::new(),
            cached_potential_loops: OnceLock::new(),
        }
    }

    /// All constructors for which `Self` is *avoidable*.
    /// This is guaranteed to be non-empty because
    /// Rust disallows coinductive types (i.e. streams, infinite-size types, etc.)
    /// Use this (when non-empty) to *allow* generation of
    /// a smaller value (in some sense).
    #[inline]
    #[must_use]
    pub fn potential_leaves(&self) -> &[IndexedCtorFn<Erased>] {
        self.cached_potential_leaves.get_or_init(|| {
            self.all_constructors
                .iter()
                .filter(|&&(_, ref c)| c.is_potential_leaf())
                .map(|&(call, ref c)| IndexedCtorFn {
                    arbitrary_fields: c.constructor.arbitrary_fields,
                    call,
                    index: c.constructor.index,
                    n_big: c.constructor.n_big(),
                })
                .collect()
        })
    }

    /// All constructors for which `Self` is *reachable*.
    /// Use this (when non-empty) to *allow* generation of
    /// a *larger* value (in some sense).
    #[inline]
    #[must_use]
    pub fn potential_loops(&self) -> &[IndexedCtorFn<Erased>] {
        self.cached_potential_loops.get_or_init(|| {
            self.all_constructors
                .iter()
                .filter(|&&(_, ref c)| c.is_potential_loop())
                .map(|&(call, ref c)| IndexedCtorFn {
                    arbitrary_fields: c.constructor.arbitrary_fields,
                    call,
                    index: c.constructor.index,
                    n_big: c.constructor.n_big(),
                })
                .collect()
        })
    }
}

impl CtorInfo {
    /// Whether this constructor is instantiable,
    /// i.e. does not contain any uninstantiable fields.
    #[inline]
    #[must_use]
    pub fn instantiable(&self, visited: &mut BTreeSet<Type>) -> bool {
        self.immediate
            .iter()
            .all(|(&ty, _)| info_by_id(ty).instantiable(visited))
    }

    /// The number of "big" types: types that either
    /// are inductive themselves or contain a big type.
    #[inline]
    #[must_use]
    pub fn n_big(&self) -> usize {
        *self.cached_n_big.get_or_init(|| {
            self.immediate
                .iter()
                .filter(|&(&ty, _)| info_by_id(ty).is_big())
                .map(|(_, count)| count.get())
                .sum()
        })
    }
}

impl PrecomputedTypeFormer {
    #[inline]
    #[must_use]
    pub fn algebraic<T>(
        all_constructors: Vec<(CtorFn<T>, CtorVertex)>,
        eliminator: ElimFn<T>,
    ) -> Self {
        Self::Algebraic(AlgebraicTypeFormer::new::<T>(all_constructors, eliminator))
    }

    #[inline]
    #[must_use]
    pub fn literal<T>(
        deserialize: fn(&str) -> Option<T>,
        generate: for<'prng> fn(&'prng mut WyRand) -> T,
        serialize: fn(&T) -> String,
        shrink: fn(T) -> Box<dyn Iterator<Item = T>>,
    ) -> Self {
        Self::Literal {
            // SAFETY: Same size, still a function pointer with the same arguments.
            deserialize: unsafe {
                mem::transmute::<fn(&str) -> Option<T>, fn(&str) -> Option<Erased>>(deserialize)
            },
            // SAFETY: Same size, still a function pointer with the same arguments.
            generate: unsafe {
                mem::transmute::<
                    for<'prng> fn(&'prng mut WyRand) -> T,
                    for<'prng> fn(&'prng mut WyRand) -> Erased,
                >(generate)
            },
            // SAFETY: Same size, still a function pointer with the same arguments.
            serialize: unsafe {
                mem::transmute::<for<'t> fn(&'t T) -> String, for<'t> fn(&'t Erased) -> String>(
                    serialize,
                )
            },
            // SAFETY: Same size, still a function pointer.
            shrink: unsafe {
                mem::transmute::<
                    fn(T) -> Box<dyn Iterator<Item = T>>,
                    fn(Erased) -> Box<dyn Iterator<Item = Erased>>,
                >(shrink)
            },
        }
    }
}

impl TypeInfo {
    /// Whether this type is instantiable,
    /// i.e. has at least one instantiable constructor.
    #[inline]
    #[must_use]
    pub fn instantiable(&self, visited: &mut BTreeSet<Type>) -> bool {
        if !visited.insert(self.vertex.ty) {
            return true;
        }
        match self.type_former {
            PrecomputedTypeFormer::Literal { .. } => true,
            PrecomputedTypeFormer::Algebraic(ref algebraic) => algebraic
                .all_constructors
                .iter()
                .any(|&(_, ref c)| c.constructor.instantiable(visited)),
        }
    }

    /// Whether `Self` either is inductive or contains a big type.
    #[inline]
    #[must_use]
    #[expect(clippy::missing_panics_doc, reason = "internal invariants")]
    pub fn is_big(&self) -> bool {
        *self.cached_big.get_or_init(|| {
            if self.vertex.is_inductive() {
                return true;
            }
            let PrecomputedTypeFormer::Algebraic(ref alg) = self.type_former else {
                return false;
            };
            alg.all_constructors.iter().any(|&(_, ref v)| {
                v.constructor.immediate.iter().any(|(&ty, _)| {
                    assert_ne!(
                        ty, self.vertex.ty,
                        "internal `pbt` error: inductivity calculation error",
                    );
                    info_by_id(ty).is_big()
                })
            })
        })
    }
}

#[expect(
    clippy::diverging_sub_expression,
    clippy::panic,
    reason = "internal invariant that ought to panic before causing damage"
)]
impl Pbt for Erased {
    #[inline]
    fn register_all_immediate_dependencies(
        _visited: &mut BTreeSet<Type>,
        _sccs: &mut StronglyConnectedComponents,
    ) {
        panic!("internal `pbt` error: do not call `Pbt` methods on `Erased`")
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        panic!("internal `pbt` error: do not call `Pbt` methods on `Erased`")
    }

    #[inline]
    #[expect(
        unreachable_code,
        unused_variables,
        reason = "to constrain the anonymous return type"
    )]
    fn visit_deep<V: Pbt>(&self) -> impl Iterator<Item = V> {
        let panic: iter::Empty<_> =
            panic!("internal `pbt` error: do not call `Pbt` methods on `Erased`");
        panic
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl Clone for Terms {
    #[inline]
    fn clone(&self) -> Self {
        let clone = info_by_id(self.ty).erased_bucket_ops.clone;
        Self {
            terms: clone(&self.terms),
            ty: self.ty,
        }
    }
}

impl fmt::Debug for Terms {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (info_by_id(self.ty).erased_bucket_ops.debug)(&self.terms, f)
    }
}

impl Drop for Terms {
    #[inline]
    fn drop(&mut self) {
        (info_by_id(self.ty).erased_bucket_ops.drop)(mem::take(&mut self.terms))
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl Eq for Terms {}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl PartialEq for Terms {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty && (info_by_id(self.ty).erased_bucket_ops.eq)(&self.terms, &other.terms)
    }
}

impl Terms {
    #[inline]
    pub fn shrink(self) -> impl Iterator<Item = Self> {
        let ty = self.ty;
        let shrink = info_by_id(ty).erased_bucket_ops.shrink;
        // SAFETY: Not double-dropped b/c `mem::forget` below.
        let terms = unsafe { ptr::read(ptr::from_ref(&self.terms)) };
        #[expect(
            clippy::mem_forget,
            reason = "to avoid double-dropping the `Vec<Erased>` moved above"
        )]
        let () = mem::forget(self);
        shrink(terms).map(move |terms| Self { terms, ty })
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl Clone for TermsOfVariousTypes {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            map: self
                .map
                .iter()
                .map(|(&ty, terms)| (ty, terms.clone()))
                .collect(),
        }
    }
}

impl fmt::Debug for TermsOfVariousTypes {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();
        for (&ty, terms) in &self.map {
            let info = info_by_id(ty);
            map.entry(&info.name, terms);
        }
        map.finish()
    }
}

impl Drop for TermsOfVariousTypes {
    #[inline]
    fn drop(&mut self) {
        for (_, terms) in mem::take(&mut self.map) {
            drop(terms);
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl Eq for TermsOfVariousTypes {}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl PartialEq for TermsOfVariousTypes {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.map == other.map
    }
}

impl TermsOfVariousTypes {
    #[inline]
    #[must_use]
    /// Borrow the bucket for one concrete type without exposing the erased storage.
    pub fn get<T: Pbt>(&self) -> Option<&[T]> {
        let id = type_of::<T>();
        let v = self.map.get(&id)?;
        let v: *const Vec<Erased> = ptr::from_ref(&v.terms);
        let v: *const Vec<T> = v.cast();
        // SAFETY: Undoing the earlier `transmute` in `push` (the only entry point);
        // no operations are ever performed on the erased `Vec<Erased>` state.
        let v = unsafe { v.as_ref_unchecked() };
        Some(v)
    }

    /// Mutably borrow the list of terms of a given type.
    #[inline]
    #[must_use]
    fn get_mut<T: Pbt>(&mut self) -> Option<&mut Vec<T>> {
        let id = type_of::<T>();
        let v = self.map.get_mut(&id)?;
        let v: *mut Vec<Erased> = ptr::from_mut(&mut v.terms);
        let v: *mut Vec<T> = v.cast();
        // SAFETY: Undoing the earlier `transmute` in `push` (the only entry point);
        // no operations are ever performed on the erased `Vec<Erased>` state.
        let v = unsafe { v.as_mut_unchecked() };
        Some(v)
    }

    #[inline]
    #[must_use]
    /// Return whether all term buckets have been drained.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
            || {
                debug_assert!(
                    !self.map.iter().any(|(_, v)| v.terms.is_empty()),
                    "internal `pbt` error: `TermsOfVariousTypes` contained an empty vector; it should have been removed from the map after `pop`",
                );
                false
            }
    }

    /// Remove the last-pushed term of a given type (usually inferred).
    /// # Panics
    /// If no terms of that type remain.
    #[inline]
    pub fn must_pop<T: Pbt>(&mut self) -> T {
        match self.pop::<T>() {
            Some(t) => t,
            #[expect(clippy::panic, reason = "internal invariants")]
            None => panic!(
                "internal `pbt` error: popped too many `{}`s",
                type_name::<T>(),
            ),
        }
    }

    #[inline]
    #[must_use]
    /// Create an empty set of erased term buckets.
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    /// Remove the last-pushed term of a given type (usually inferred).
    #[inline]
    #[expect(
        clippy::missing_panics_doc,
        reason = "won't panic b/c internal invariants"
    )]
    pub fn pop<T: Pbt>(&mut self) -> Option<T> {
        let v: &mut Vec<T> = self.get_mut()?;
        let opt: Option<T> = v.pop();
        if opt.is_none() || v.is_empty() {
            #[expect(
                clippy::expect_used,
                clippy::unwrap_in_result,
                reason = "won't panic b/c internal invariants"
            )]
            let v = self
                .map
                .remove(&type_of::<T>())
                .expect("internal `pbt` error: failed to remove empty vector of terms");
            drop(v)
        }
        opt
    }

    #[inline]
    /// Remove and serialize the most recently pushed term from the bucket keyed by `ty`.
    pub fn pop_serialize_by_id(&mut self, ty: Type) -> Option<CachedTerm> {
        let terms = self.map.get_mut(&ty)?;
        let term = (info_by_id(ty).erased_bucket_ops.pop_serialize)(&mut terms.terms)?;
        if terms.terms.is_empty() {
            let removed = self.map.remove(&ty)?;
            drop(removed)
        }
        Some(term)
    }

    #[inline]
    /// Push one typed term into the bucket keyed by its concrete `TypeId`.
    pub fn push<T: Pbt>(&mut self, t: T) {
        let id = type_of::<T>();
        let terms = match self.map.entry(id) {
            btree_map::Entry::Occupied(entry) => entry.into_mut(),
            btree_map::Entry::Vacant(entry) => {
                let _info = info::<T>();
                entry.insert(Terms {
                    // SAFETY: Never used in its erased form, and
                    // `Vec<_>`s all have the same size+alignment.
                    terms: unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(vec![]) },
                    ty: id,
                })
            }
        };
        let v: *mut Vec<Erased> = ptr::from_mut(&mut terms.terms);
        let v: *mut Vec<T> = v.cast();
        // SAFETY: Undoing the earlier `transmute` in `push` (the only entry point);
        // no operations are ever performed on the erased `Vec<Erased>` state.
        let v = unsafe { v.as_mut_unchecked() };
        v.push(t)
    }

    #[inline]
    /// Breadth-first shrink this collection of erased term buckets.
    pub fn shrink(self) -> impl Iterator<Item = Self> {
        let mut this = self;
        let map = mem::take(&mut this.map);

        // The general idea here is "breadth-first iteration":
        // given some collection of iterators,
        // call `next` (at most) *once* for each
        // before closing the loop and iterating once more,
        // until all iterators have been exhausted.
        //
        // This leaves open the question of what to do with
        // the elements that were *not* just `next`'d:
        // in the specific case of shrinking, I tentatively believe
        // it's both acceptable and probably optimal to
        // pin those elements to their *original* values,
        // leaving only one degree of freedom per iteration.
        //
        // It's also worth noting that we have
        // two layers of collections here:
        //   1. the map from types to collections of terms, and
        //   2. the collections of terms themselves.
        // So, for any given iteration, we pick *one* type,
        // and we vary only *one* term of that type.
        // Since shrinking (if implemented well)
        // cuts about half the remaining "size" of the value
        // per iteration and restarts on success,
        // this should be pretty efficient in practice,
        // given the unsolvable nature of the real
        // global optimization problem.

        // Split the map into keys and *iterators over* values
        // so we can apply breadth-first iteration to the latter:
        let (keys, value_iterators): (Vec<Type>, Vec<_>) = map
            .into_iter()
            .map(|(k, v)| (k, (v.clone(), v.shrink())))
            .unzip();

        // Transpose from a collection of iterators
        // to an iterator over collections:
        let iterator_over_values = breadth_first_transpose(value_iterators);

        // Restore each collection's associated type
        // (note that this is almost comically dangerous):
        let iterator_over_maps = iterator_over_values
            .map(move |values: Vec<Terms>| keys.iter().copied().zip(values).collect());

        iterator_over_maps.map(|map| Self { map })
    }
}

impl Default for TermsOfVariousTypes {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Type {
    /// The opaque Rust ID for this type.
    #[inline]
    #[must_use]
    pub fn id(self) -> TypeId {
        self.0
    }
}

impl Vertex {
    /// Whether `Self` is inductive.
    /// Internally, this asks whether its
    /// strongly connected component is nontrivial.
    #[inline]
    #[must_use]
    #[expect(
        clippy::expect_used,
        clippy::missing_panics_doc,
        clippy::panic,
        reason = "internal invariants"
    )]
    pub fn is_inductive(&self) -> bool {
        *self.cached_inductivity.get_or_init(|| {
            let mut sccs = _sccs()
                .write()
                .expect("internal `pbt` error: SCC lock poisoned");
            let () = sccs.tarjan_dfs(self.ty, &mut BTreeMap::new(), &mut vec![], &mut 0);
            let Some(root) = sccs.root(self.ty) else {
                panic!(
                    "internal `pbt` error: type `{:?}` absent from SCC graph",
                    self.ty,
                )
            };
            let Some(&scc::Node::Root(scc::Metadata { cardinality, .. })) = sccs.get(&root) else {
                panic!("internal `pbt` error: `scc::root` is not idempotent")
            };
            cardinality.is_some()
        })
    }
}

impl CtorVertex {
    /// Whether `Self` is *unreachable*.
    #[inline]
    #[must_use]
    pub fn is_guaranteed_leaf(&self) -> bool {
        self.constructor.instantiable(&mut BTreeSet::new()) && !self.is_potential_loop()
    }

    /// Whether `Self` is *unavoidable*.
    #[inline]
    #[must_use]
    pub fn is_guaranteed_loop(&self) -> bool {
        self.constructor.instantiable(&mut BTreeSet::new())
            && self.vertex.unavoidable.contains(&self.vertex.ty)
    }

    /// Whether `Self` is inductive.
    #[inline]
    #[must_use]
    #[expect(
        clippy::expect_used,
        clippy::missing_panics_doc,
        clippy::panic,
        reason = "internal invariants"
    )]
    pub fn is_inductive(&self) -> bool {
        *self.vertex.cached_inductivity.get_or_init(|| {
            let mut sccs = _sccs()
                .write()
                .expect("internal `pbt` error: SCC lock poisoned");
            let () = sccs.tarjan_dfs(self.vertex.ty, &mut BTreeMap::new(), &mut vec![], &mut 0);
            let Some(self_root) = sccs.root(self.vertex.ty) else {
                panic!(
                    "internal `pbt` error: type `{:?}` absent from SCC graph",
                    self.vertex.ty,
                )
            };
            for (&ty, _count) in self.constructor.immediate.iter() {
                let Some(root) = sccs.root(ty) else {
                    panic!("internal `pbt` error: type `{ty:?}` absent from SCC graph")
                };
                if root == self_root {
                    return true;
                }
            }
            false
        })
    }

    /// Whether `Self` is *avoidable*.
    #[inline]
    #[must_use]
    pub fn is_potential_leaf(&self) -> bool {
        self.constructor.instantiable(&mut BTreeSet::new()) && !self.is_guaranteed_loop()
    }

    /// Whether `Self` is *reachable*.
    #[inline]
    #[must_use]
    pub fn is_potential_loop(&self) -> bool {
        self.constructor.instantiable(&mut BTreeSet::new()) && self.is_inductive()
    }
}

impl fmt::Debug for Type {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match try_info_by_id(*self) {
            None => write!(f, "[unregistered type with ID {:?}]", self.0),
            Some(info) => f.write_str(info.name),
        }
    }
}

/// Given some collection of iterators,
/// call `next` (at most) *once* for each
/// before closing the loop and iterating once more,
/// until all iterators have been exhausted.
#[inline]
pub(crate) fn breadth_first_transpose<I: Iterator<Item: Clone>>(
    initial_values_and_iterators: Vec<(I::Item, I)>,
) -> impl Iterator<Item = Vec<I::Item>> {
    let (initial_values, mut iterators): (Vec<I::Item>, Vec<I>) =
        initial_values_and_iterators.into_iter().unzip();
    // Q about the above: Why not just take two vectors?
    // A: to constrain the length of both to be equal.
    // TODO: maybe just add an assertion to avoid reallocating?
    let mut any_iterators_still_active_this_round = false;
    let mut i = 0;
    iter::from_fn(move || {
        'restart: loop {
            #[expect(clippy::mixed_read_write_in_expression, reason = "initialized above")]
            let Some(iterator) = iterators.get_mut(i) else {
                if !any_iterators_still_active_this_round {
                    return None;
                }
                any_iterators_still_active_this_round = false;
                i = 0;
                continue 'restart;
            };
            {
                #![expect(
                    clippy::arithmetic_side_effects,
                    reason = "bounded by `Vec::len`, in turn by system hardware, matching the capacity of `usize` by definition"
                )]
                i += 1; // for next time
            }
            if let Some(next) = iterator.next() {
                any_iterators_still_active_this_round = true;
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "safely incremented earlier, so decrementing is safe"
                )]
                return Some(
                    initial_values
                        .iter()
                        .take(i - 1)
                        .cloned()
                        .chain(iter::once(next))
                        .chain(initial_values.iter().skip(i).cloned())
                        .collect(),
                );
            }
        }
    })
}

/// Build the erased bucket operations for one concrete term type.
#[inline]
fn erased_bucket_ops<T: Pbt>() -> ErasedBucketOps {
    ErasedBucketOps {
        clone: |v| {
            // SAFETY: Undoing an earlier pointer cast; the bucket is keyed by `T`.
            let erased = unsafe { &*(ptr::from_ref(v).cast::<Vec<T>>()) }.clone();
            // SAFETY: Never used in its erased form, and
            // `Vec<_>`s all have the same size+alignment.
            unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(erased) }
        },
        debug: |v, f| {
            // SAFETY: Undoing an earlier pointer cast; the bucket is keyed by `T`.
            fmt::Debug::fmt(unsafe { &*(ptr::from_ref(v).cast::<Vec<T>>()) }, f)
        },
        // SAFETY: Undoing an earlier `transmute`.
        drop: |v| mem::drop(unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(v) }),
        eq: |lhs, rhs| {
            <Vec<T> as PartialEq>::eq(
                // SAFETY: Undoing an earlier pointer cast; the bucket is keyed by `T`.
                unsafe { &*(ptr::from_ref(lhs).cast::<Vec<T>>()) },
                // SAFETY: Undoing an earlier pointer cast; the bucket is keyed by `T`.
                unsafe { &*(ptr::from_ref(rhs).cast::<Vec<T>>()) },
            )
        },
        pop_serialize: |v| {
            // SAFETY: Undoing the earlier pointer cast in `push`; the bucket is keyed by `T`.
            let typed = unsafe { &mut *(ptr::from_mut(v).cast::<Vec<T>>()) };
            typed.pop().map(|t| cache::serialize_term(&t))
        },
        shrink: |v| {
            // SAFETY: Never used in its erased form, and
            // `Box<_>`es all have the same size+alignment.
            let v = unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(v) };
            let vec_of_iterators = v.into_iter().map(|t| (t.clone(), shrink(t))).collect();
            let iterator_of_vecs = breadth_first_transpose(vec_of_iterators);
            let iterator_of_erased_vecs = iterator_of_vecs.map(|v| {
                // SAFETY: Never used in its erased form, and
                // `Vec<_>`s all have the same size+alignment.
                unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(v) }
            });
            Box::new(iterator_of_erased_vecs)
        },
    }
}

/// Compute the complete reflection payload for one type within the current
/// serialized registration traversal.
///
/// This computes the registry payload and the corresponding SCC node, but it
/// does not publish either one globally. Publication happens in `register_inner`
/// once the caller has confirmed that the type is still absent.
#[inline]
#[expect(clippy::too_many_lines, reason = "TODO: refactor")]
fn compute_type_registration<T: Pbt>(
    mut visited: BTreeSet<Type>,
    sccs: &mut StronglyConnectedComponents,
) -> ComputedTypeRegistration {
    let () = T::register_all_immediate_dependencies(&mut visited, sccs);
    let self_ty = type_of::<T>();
    let type_former = T::type_former();
    let (shallow_ctors, eliminator) = match type_former {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules,
            elimination_rule,
        }) => (introduction_rules, elimination_rule),
        TypeFormer::Literal(Literal {
            deserialize,
            generate,
            serialize,
            shrink,
        }) => {
            return ComputedTypeRegistration {
                info: TypeInfo {
                    erased_bucket_ops: erased_bucket_ops::<T>(),
                    cached_big: OnceLock::from(false),
                    deserialize_cached_term_into_buckets: deserialize_cached_term_into_buckets::<T>,
                    name: type_name::<T>(),
                    trivial: true,
                    type_former: PrecomputedTypeFormer::literal(
                        deserialize,
                        generate,
                        serialize,
                        shrink,
                    ),
                    vertex: Vertex {
                        cached_inductivity: OnceLock::from(false),
                        ty: self_ty,
                        unavoidable: BTreeSet::new(),
                    },
                },
                scc_node: scc::Node::Root(scc::Metadata {
                    cardinality: None,
                    edges: BTreeSet::new(),
                    ty: self_ty,
                }),
            };
        }
    };

    // Necessary to do this here, since we don't want *transitive* dependencies;
    // we care only whether this type wraps a single other type,
    // not anything about the type that's being wrapped or any transitive dependencies.
    let trivial = match *shallow_ctors.as_slice() {
        [] => true,
        [ref singleton] => {
            let n_fields: usize = singleton
                .immediate_dependencies
                .iter()
                .map(|(_, count)| count.get())
                .sum();
            n_fields <= 1
        }
        _ => false,
    };

    let mut constructors: Vec<(CtorFn<T>, CtorVertex)> = vec![];
    let mut immediately_reachable: BTreeSet<Type> = BTreeSet::new();
    let mut unavoidable: Option<BTreeSet<Type>> = None;
    for (
        i,
        IntroductionRule {
            arbitrary_fields,
            call,
            immediate_dependencies,
        },
    ) in shallow_ctors.into_iter().enumerate()
    {
        let mut ctor_unavoidable = BTreeSet::new();
        for (&ty, _count) in immediate_dependencies.iter() {
            let _: bool = ctor_unavoidable.insert(ty);
            if visited.contains(&ty) {
                // means this type is either coinductive
                // or represented differently than laid out in memory
                // (e.g. `Vec<_>` takes a `Self` (tail) and a `T` (head));
                // in that case, recursing will not matter at all,
                // since we're already at `Self` and this is a shallow operation
                let _: bool = ctor_unavoidable.insert(ty);
            } else {
                let info = info_by_id(ty);
                let () = ctor_unavoidable.extend(&info.vertex.unavoidable);
            }
        }
        let () = immediately_reachable.extend(ctor_unavoidable.iter());
        unavoidable = Some(unavoidable.map_or_else(
            || ctor_unavoidable.clone(),
            |mut unavoidable| {
                // Multiset::intersection(&unavoidable, &ctor_unavoidable)
                let () = unavoidable.retain(|ty| ctor_unavoidable.contains(ty));
                unavoidable
            },
        ));
        let deps = CtorVertex {
            constructor: CtorInfo {
                arbitrary_fields,
                cached_n_big: OnceLock::new(),
                immediate: immediate_dependencies,
                #[expect(clippy::expect_used, reason = "extremely unlikely")]
                index: ONE
                    .checked_add(i)
                    .expect("internal `pbt` error: more than `usize::MAX` constructors"),
            },
            vertex: Vertex {
                cached_inductivity: OnceLock::new(),
                ty: self_ty,
                unavoidable: ctor_unavoidable,
            },
        };
        let () = constructors.push((call, deps));
    }

    ComputedTypeRegistration {
        info: TypeInfo {
            erased_bucket_ops: erased_bucket_ops::<T>(),
            cached_big: OnceLock::new(),
            deserialize_cached_term_into_buckets: deserialize_cached_term_into_buckets::<T>,
            name: type_name::<T>(),
            trivial,
            type_former: PrecomputedTypeFormer::algebraic(constructors, eliminator),
            vertex: Vertex {
                cached_inductivity: OnceLock::new(),
                ty: self_ty,
                unavoidable: unavoidable.unwrap_or_default(),
            },
        },
        scc_node: scc::Node::Root(scc::Metadata {
            cardinality: (immediately_reachable.contains(&self_ty))
                .then_some(const { NonZero::new(1).unwrap() }),
            edges: immediately_reachable,
            ty: self_ty,
        }),
    }
}

/// Register a single type inside an already-serialized registration traversal.
#[inline]
fn register_inner<T: Pbt>(visited: BTreeSet<Type>, sccs: &mut StronglyConnectedComponents) {
    let id = type_of::<T>();
    if visited.contains(&id) || try_info_by_id(id).is_some() {
        return;
    }

    let ComputedTypeRegistration { info, scc_node } = compute_type_registration::<T>(visited, sccs);
    let pinned = _registry().pin();
    let _: &Arc<TypeInfo> = pinned.get_or_insert_with(id, || Arc::new(info));
    let overwritten = sccs.insert(id, scc_node);
    debug_assert!(
        overwritten.is_none(),
        "internal `pbt` error: duplicate SCC registration for {id:?}",
    );
}

/// Register a type with the global registry of type dependency information.
#[inline]
pub fn register<T: Pbt>(visited: BTreeSet<Type>, sccs: &mut StronglyConnectedComponents) {
    let id = type_of::<T>();
    if visited.contains(&id) {
        return;
    }
    let () = register_inner::<T>(visited, sccs);
}

/// Get a handle to the global type-information registry without trying to lock it.
/// **Do not use this unless you are a `pbt` maintainer.**
#[inline]
#[must_use]
pub fn _registry() -> &'static papaya::HashMap<Type, Arc<TypeInfo>, RandomState> {
    static REGISTRY: LazyLock<papaya::HashMap<Type, Arc<TypeInfo>, RandomState>> =
        LazyLock::new(|| papaya::HashMap::with_hasher(RandomState::with_seed(usize::from(SEED))));
    LazyLock::force(&REGISTRY)
}

/// Get a handle to the global strongly connected component graph without trying to lock it.
/// **Do not use this unless you are a `pbt` maintainer.**
#[inline]
#[must_use]
pub fn _sccs() -> &'static RwLock<StronglyConnectedComponents> {
    static SCCS: LazyLock<RwLock<StronglyConnectedComponents>> =
        LazyLock::new(|| RwLock::new(StronglyConnectedComponents::new()));
    LazyLock::force(&SCCS)
}

/// Get type-level characteristics of a type,
/// or compute and cache them if they
/// haven't yet been determined.
/// # Panics
/// If registration completes but the just-registered type cannot be found in the registry.
#[inline]
#[must_use]
pub fn info<T: Pbt>() -> Arc<TypeInfo> {
    let mut sccs = _sccs().write().unwrap_or_else(PoisonError::into_inner);
    let () = register_inner::<T>(BTreeSet::new(), &mut sccs);
    match try_info_by_id(type_of::<T>()) {
        Some(info) => info,
        #[expect(
            clippy::panic,
            reason = "registration must publish the just-computed type info"
        )]
        None => panic!("internal `pbt` error: just-registered type missing"),
    }
}

/// Get type-level characteristics of a type by its unique but opaque type ID.
/// # Panics
/// If the type has not yet been registered with `pbt`.
#[inline]
#[must_use]
pub fn info_by_id(ty: Type) -> Arc<TypeInfo> {
    #[expect(clippy::panic, reason = "internal invariants; violation should panic")]
    try_info_by_id(ty).unwrap_or_else(|| {
        panic!(
            "internal `pbt` error: unregistered type with ID `{:?}` (registered so far: {:#?})",
            ty.id(),
            _registry()
                .pin()
                .iter()
                .map(|(&id, info)| (id, info.vertex.ty.id()))
                .collect::<BTreeMap<Type, TypeId>>(),
        )
    })
}

/// Get type-level characteristics of a type by its unique but opaque type ID.
/// Returns `None` if the type has not yet been registered with `pbt`.
#[inline]
pub fn try_info_by_id(id: Type) -> Option<Arc<TypeInfo>> {
    let pinned = _registry().pin();
    pinned.get(&id).map(Arc::clone)
}

#[inline]
#[must_use]
pub fn type_of<T: Pbt>() -> Type {
    Type(TypeId::of::<T>())
}
