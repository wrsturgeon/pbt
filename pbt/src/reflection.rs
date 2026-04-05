use {
    crate::{
        SEED,
        construct::{
            Algebraic, Construct, CtorFn, ElimFn, IndexedCtorFn, IntroductionRule, Literal,
            TypeFormer,
        },
        multiset::Multiset,
        scc::{self, StronglyConnectedComponents},
        shrink::shrink,
        size::Size,
    },
    ahash::RandomState,
    core::{
        any::{TypeId, type_name},
        fmt, iter, mem,
        num::NonZero,
        ptr,
    },
    std::{
        collections::{BTreeMap, BTreeSet},
        sync::{Arc, LazyLock, OnceLock, RwLock},
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

/// An erased term of some type that implements `Clone + Debug + Eq`.
#[non_exhaustive]
pub struct Terms {
    pub clone: for<'t> fn(&'t Vec<Erased>) -> Vec<Erased>,
    pub debug: for<'t, 'm, 'f> fn(&'t Vec<Erased>, &'m mut fmt::Formatter<'f>) -> fmt::Result,
    pub drop: fn(Vec<Erased>),
    pub eq: for<'lhs, 'rhs> fn(&'lhs Vec<Erased>, &'rhs Vec<Erased>) -> bool,
    pub shrink: fn(Vec<Erased>) -> Box<dyn Iterator<Item = Vec<Erased>>>,
    pub terms: Vec<Erased>,
}

/// A map from types to ordered collections of terms of those types.
/// This is used e.g. for constructors:
/// each constructor knows the multiset of types it needs to fill its fields,
/// so it can request exactly enough terms of various types to do so.
#[non_exhaustive]
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TermsOfVariousTypes {
    /// A map from types to ordered collections of terms of those types.
    pub map: BTreeMap<Type, Terms>,
}

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
#[derive(Debug)]
pub struct TypeInfo {
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

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CtorInfo {
    /// The multiset of types necessary to call this constructor.
    pub immediate: Multiset<Type>,
    /// 1-indexed constructor/variant index.
    pub index: NonZero<usize>,
}

#[non_exhaustive]
#[derive(Debug)]
pub struct AlgebraicTypeFormer {
    /// The exhaustive disjoint set of methods
    /// to construct a term of this type,
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
        generate: for<'prng> fn(&'prng mut WyRand) -> Erased,
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
                    call,
                    index: c.constructor.index,
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
                    call,
                    index: c.constructor.index,
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
                    call,
                    index: c.constructor.index,
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
                    call,
                    index: c.constructor.index,
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
    pub fn instantiable(&self) -> bool {
        self.immediate
            .iter()
            .all(|(&ty, _)| info_by_id(ty).instantiable())
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
        generate: for<'prng> fn(&'prng mut WyRand) -> T,
        shrink: fn(T) -> Box<dyn Iterator<Item = T>>,
    ) -> Self {
        Self::Literal {
            // SAFETY: Same size, still a function pointer with the same arguments.
            generate: unsafe {
                mem::transmute::<
                    for<'prng> fn(&'prng mut WyRand) -> T,
                    for<'prng> fn(&'prng mut WyRand) -> Erased,
                >(generate)
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
    pub fn instantiable(&self) -> bool {
        match self.type_former {
            PrecomputedTypeFormer::Literal { .. } => true,
            PrecomputedTypeFormer::Algebraic(ref algebraic) => algebraic
                .all_constructors
                .iter()
                .any(|&(_, ref c)| c.constructor.instantiable()),
        }
    }
}

#[expect(
    clippy::diverging_sub_expression,
    clippy::panic,
    reason = "internal invariant that ought to panic before causing damage"
)]
impl Construct for Erased {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        panic!("internal `pbt` error: do not call `Construct` methods on `Erased`")
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {
        panic!("internal `pbt` error: do not call `Construct` methods on `Erased`")
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        panic!("internal `pbt` error: do not call `Construct` methods on `Erased`")
    }

    #[inline]
    #[expect(
        unreachable_code,
        unused_variables,
        reason = "to constrain the anonymous return type"
    )]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        let panic: iter::Empty<_> =
            panic!("internal `pbt` error: do not call `Construct` methods on `Erased`");
        panic
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl Clone for Terms {
    #[inline]
    fn clone(&self) -> Self {
        let Self {
            clone,
            debug,
            drop,
            eq,
            shrink,
            ref terms,
        } = *self;
        Self {
            clone,
            debug,
            drop,
            eq,
            shrink,
            terms: (self.clone)(terms),
        }
    }
}

impl fmt::Debug for Terms {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.debug)(&self.terms, f)
    }
}

impl Drop for Terms {
    #[inline]
    fn drop(&mut self) {
        (self.drop)(mem::take(&mut self.terms))
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl Eq for Terms {}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl PartialEq for Terms {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        (self.eq)(&self.terms, &other.terms)
    }
}

impl Terms {
    #[inline]
    pub fn shrink(self) -> impl Iterator<Item = Self> {
        let Self {
            clone,
            debug,
            drop,
            eq,
            shrink,
            ref terms,
        } = self;
        // SAFETY: Not double-dropped b/c `mem::forget` below.
        let terms = unsafe { ptr::read(terms) };
        #[expect(
            clippy::mem_forget,
            reason = "to avoid double-dropping the `Vec<Erased>` moved above"
        )]
        let () = mem::forget(self);
        shrink(terms).map(move |terms| Self {
            clone,
            debug,
            drop,
            eq,
            shrink,
            terms,
        })
    }
}

impl TermsOfVariousTypes {
    #[inline]
    #[must_use]
    pub fn get<T: Construct>(&self) -> Option<&[T]> {
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
    fn get_mut<T: Construct>(&mut self) -> Option<&mut Vec<T>> {
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
    pub fn must_pop<T: Construct>(&mut self) -> T {
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
    pub fn pop<T: Construct>(&mut self) -> Option<T> {
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
    #[expect(clippy::transmute_ptr_to_ptr, reason = "maximally explicit types")]
    pub fn push<T: Construct>(&mut self, t: T) {
        let id = type_of::<T>();
        let terms = self.map.entry(id).or_insert_with(|| Terms {
            // SAFETY: Never used in its erased form, and
            // `Vec<_>`s all have the same size+alignment.
            terms: unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(vec![]) },
            clone: |v| {
                // SAFETY: Undoing an earlier `transmute`.
                let erased = unsafe { mem::transmute::<&Vec<Erased>, &Vec<T>>(v) }.clone();
                // SAFETY: Never used in its erased form, and
                // `Vec<_>`s all have the same size+alignment.
                unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(erased) }
            },
            // SAFETY: Undoing an earlier `transmute`.
            debug: |v, f| fmt::Debug::fmt(unsafe { mem::transmute::<&Vec<Erased>, &Vec<T>>(v) }, f),
            // SAFETY: Undoing an earlier `transmute`.
            drop: |v| mem::drop(unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(v) }),
            eq: |lhs, rhs| {
                <Vec<T> as PartialEq>::eq(
                    // SAFETY: Undoing an earlier `transmute`.
                    unsafe { mem::transmute::<&Vec<Erased>, &Vec<T>>(lhs) },
                    // SAFETY: Undoing an earlier `transmute`.
                    unsafe { mem::transmute::<&Vec<Erased>, &Vec<T>>(rhs) },
                )
            },
            shrink: |v| {
                // SAFETY: Never used in its erased form, and
                // `Box<_>`es all have the same size+alignment.
                let v = unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(v) };
                let vec_of_iterators = v.into_iter().map(|t| (t.clone(), shrink(t))).collect(); // beautiful -- rust <3
                let iterator_of_vecs = breadth_first_transpose(vec_of_iterators);
                let iterator_of_erased_vecs = iterator_of_vecs.map(|v| {
                    // SAFETY: Never used in its erased form, and
                    // `Box<_>`es all have the same size+alignment.
                    unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(v) }
                });
                Box::new(iterator_of_erased_vecs)
            },
        });
        let v: *mut Vec<Erased> = ptr::from_mut(&mut terms.terms);
        let v: *mut Vec<T> = v.cast();
        // SAFETY: Undoing the earlier `transmute` in `push` (the only entry point);
        // no operations are ever performed on the erased `Vec<Erased>` state.
        let v = unsafe { v.as_mut_unchecked() };
        v.push(t)
    }

    #[inline]
    pub fn shrink(self) -> impl Iterator<Item = Self> {
        let Self { map } = self;

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
            .map(|(k, v)| {
                (
                    k,
                    // Ask `Terms::shrink` to do the heavy lifting,
                    // giving us a vector of *iterators over* collections of terms:
                    (v.clone(), v.shrink()),
                )
            })
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
        self.constructor.instantiable() && !self.is_potential_loop()
    }

    /// Whether `Self` is *unavoidable*.
    #[inline]
    #[must_use]
    pub fn is_guaranteed_loop(&self) -> bool {
        self.constructor.instantiable() && self.vertex.unavoidable.contains(&self.vertex.ty)
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
        self.constructor.instantiable() && !self.is_guaranteed_loop()
    }

    /// Whether `Self` is *reachable*.
    #[inline]
    #[must_use]
    pub fn is_potential_loop(&self) -> bool {
        self.constructor.instantiable() && self.is_inductive()
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

/// Register a type with the global registry of type dependency information.
/// If this function is called, then the function is *not* already in the registry,
/// and the return value of this function will be *automatically* added to the registry.
/// Do not attempt either operation manually from within this function.
#[inline]
#[expect(clippy::too_many_lines, reason = "TODO: refactor")]
fn compute_type_info<T: Construct>(mut visited: BTreeSet<Type>) -> TypeInfo {
    let self_ty = type_of::<T>();
    let not_already_visited = visited.insert(self_ty);
    assert!(
        not_already_visited,
        "internal `pbt` error: `visited` already contained `Self = {}` (`visited` was {visited:?})",
        type_name::<T>(),
    );

    let () = T::register_all_immediate_dependencies(&visited);

    let type_former = T::type_former();
    let (shallow_ctors, eliminator) = match type_former {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules,
            elimination_rule,
        }) => (introduction_rules, elimination_rule),
        TypeFormer::Literal(Literal { generate, shrink }) => {
            #[expect(clippy::expect_used, reason = "extremely unlikely & unrecoverable")]
            let _: Option<scc::Node> = _sccs()
                .write()
                .expect("internal `pbt` error: SCC lock poisoned")
                .insert(
                    self_ty,
                    scc::Node::Root(scc::Metadata {
                        cardinality: None,
                        edges: BTreeSet::new(),
                        ty: self_ty,
                    }),
                );
            return TypeInfo {
                name: type_name::<T>(),
                trivial: true,
                type_former: PrecomputedTypeFormer::literal(generate, shrink),
                vertex: Vertex {
                    cached_inductivity: OnceLock::new(),
                    ty: self_ty,
                    unavoidable: BTreeSet::new(),
                },
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
            call,
            immediate_dependencies,
        },
    ) in shallow_ctors.into_iter().enumerate()
    {
        let mut ctor_unavoidable = BTreeSet::new();
        for (&ty, _count) in immediate_dependencies.iter() {
            let _: bool = ctor_unavoidable.insert(ty);
            if !visited.contains(&ty) {
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

    #[expect(clippy::expect_used, reason = "extremely unlikely & unrecoverable")]
    let _: Option<scc::Node> = _sccs()
        .write()
        .expect("internal `pbt` error: SCC lock poisoned")
        .insert(
            self_ty,
            scc::Node::Root(scc::Metadata {
                cardinality: (immediately_reachable.contains(&self_ty))
                    .then_some(const { NonZero::new(1).unwrap() }),
                edges: immediately_reachable,
                ty: self_ty,
            }),
        );

    TypeInfo {
        name: type_name::<T>(),
        trivial,
        type_former: PrecomputedTypeFormer::algebraic(constructors, eliminator),
        vertex: Vertex {
            cached_inductivity: OnceLock::new(),
            ty: self_ty,
            unavoidable: unavoidable.unwrap_or_default(),
        },
    }
}

/// Register a type with the global registry of type dependency information.
#[inline]
pub fn register<T: Construct>(visited: BTreeSet<Type>) {
    let id = type_of::<T>();
    if visited.contains(&id) {
        return;
    }
    let pinned = _registry().pin();
    let _: &Arc<TypeInfo> =
        pinned.get_or_insert_with(id, || Arc::new(compute_type_info::<T>(visited)));
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
#[inline]
#[must_use]
pub fn info<T: Construct>() -> Arc<TypeInfo> {
    let pinned = _registry().pin();
    let in_registry = pinned.get_or_insert_with(type_of::<T>(), || {
        Arc::new(compute_type_info::<T>(BTreeSet::new()))
    });
    Arc::clone(in_registry)
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
pub fn type_of<T: Construct>() -> Type {
    Type(TypeId::of::<T>())
}
