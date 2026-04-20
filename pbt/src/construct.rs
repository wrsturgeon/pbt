use {
    crate::{
        cache,
        multiset::Multiset,
        reflection::{
            AlgebraicTypeFormer, Erased, PrecomputedTypeFormer, TermsOfVariousTypes, Type, info,
            type_of,
        },
        search,
        size::{Size, Sizes},
    },
    core::{fmt, mem, num::NonZero, ops::Deref, ptr},
    std::collections::BTreeSet,
    wyrand::WyRand,
};

#[non_exhaustive]
#[derive(Clone, Copy, Hash)]
pub struct CtorFn<T> {
    /// Function to construct a term which is an
    /// application of this constructor to arbitrary fields.
    pub call: for<'terms> fn(&'terms mut TermsOfVariousTypes) -> Option<T>,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Hash)]
pub struct IndexedCtorFn<T> {
    /// Generate precisely enough arbitrary fields
    /// to immediately invoke this constructor.
    pub arbitrary_fields: for<'prng> fn(&'prng mut WyRand, Sizes) -> TermsOfVariousTypes,
    /// Function to invoke this constructor on a collection of fields.
    pub call: CtorFn<T>,
    /// 1-indexed constructor/variant index.
    pub index: NonZero<usize>,
    /// The number of "big" types in this constructor:
    /// types that either are inductive themselves
    /// or contain a big type.
    pub n_big: usize,
}

/// Decompose this value into a
/// constructor (by index) and
/// its associated fields.
#[non_exhaustive]
#[repr(transparent)]
#[derive(Clone, Copy, Hash)]
pub struct ElimFn<T> {
    pub call: fn(T) -> Decomposition,
}

#[derive(Clone, Debug)]
#[expect(clippy::exhaustive_structs, reason = "constructed in macros")]
pub struct Algebraic<T> {
    pub elimination_rule: ElimFn<T>,
    pub introduction_rules: Vec<IntroductionRule<T>>,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Literal<T> {
    pub deserialize: fn(&str) -> Option<T>,
    pub generate: for<'prng> fn(&'prng mut WyRand) -> T,
    pub serialize: fn(&T) -> String,
    pub shrink: fn(T) -> Box<dyn Iterator<Item = T>>,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum TypeFormer<T> {
    Algebraic(Algebraic<T>),
    Literal(Literal<T>),
}

/// Decomposition of an algebraic value into its
/// constructor index and all immediate fields.
#[derive(Debug)]
#[expect(clippy::exhaustive_structs, reason = "constructed in macros")]
pub struct Decomposition {
    /// 1-indexed constructor/variant index.
    pub ctor_idx: NonZero<usize>,
    pub fields: TermsOfVariousTypes,
}

#[derive(Clone, Debug)]
#[expect(clippy::exhaustive_structs, reason = "constructed in macros")]
pub struct IntroductionRule<T> {
    /// Generate precisely enough arbitrary fields
    /// to immediately invoke this constructor.
    pub arbitrary_fields: for<'prng> fn(&'prng mut WyRand, Sizes) -> TermsOfVariousTypes,
    /// Function to invoke this constructor on a collection of fields.
    pub call: CtorFn<T>,
    /// The multiset of types necessary to call this constructor.
    pub immediate_dependencies: Multiset<Type>,
}

pub trait Construct: 'static + Clone + fmt::Debug + Eq {
    /// Run depth-first search on the global type dependency graph.
    /// All this needs to do in practice is to
    /// let some variable, e.g. `ty`, `= ::pbt::reflection::type_of::<Self>()`,
    /// add `ty` to `visited`, then,
    /// for each `T` in the `immediate_dependencies` fields of `Self::_constructors()`,
    /// call `::pbt::reflection::register::<T>(visited)`
    /// and add `::pbt::reflection::type_of::<T>()`
    /// to a set `edges` which is then passed to
    /// `::pbt::reflection::_sccs().write().register(ty, edges)`.
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>);

    /// The exhaustive disjoint set of methods
    /// to construct a term of this type.
    fn type_former() -> TypeFormer<Self>;

    /// Visit all terms of type `V` in this abstract syntax tree.
    /// Your implementation should always follow this formula:
    /// `pbt::construct::visit_self(self).chain(... recurse into fields ...)`.
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V>;
}

impl<T> CtorFn<T> {
    #[inline]
    #[must_use]
    pub const fn erase(self) -> CtorFn<Erased> {
        // SAFETY: Same size, still a function pointer with the same arguments.
        unsafe { mem::transmute::<CtorFn<T>, CtorFn<Erased>>(self) }
    }

    #[inline]
    pub const fn new(call: for<'terms> fn(&'terms mut TermsOfVariousTypes) -> Option<T>) -> Self {
        Self { call }
    }
}

impl CtorFn<Erased> {
    /// Interpret this type-erased generator as a generator for a specific type.
    /// # Safety
    /// You'd better be damn well sure that you're specifying the right type.
    #[inline]
    #[must_use]
    pub const unsafe fn unerase<T>(
        self,
    ) -> for<'terms> fn(&'terms mut TermsOfVariousTypes) -> Option<T> {
        // SAFETY: Same size, still a function pointer with the same arguments.
        unsafe { mem::transmute::<CtorFn<Erased>, CtorFn<T>>(self) }.call
    }
}

impl<T> fmt::Debug for CtorFn<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(|terms| ...)")
    }
}

impl<T> Deref for CtorFn<T> {
    type Target = for<'terms> fn(&'terms mut TermsOfVariousTypes) -> Option<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.call
    }
}

impl<T> Deref for IndexedCtorFn<T> {
    type Target = CtorFn<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.call
    }
}

impl<T> ElimFn<T> {
    #[inline]
    #[must_use]
    pub const fn erase(self) -> ElimFn<Erased> {
        // SAFETY: Same size, still a function pointer with the same arguments.
        unsafe { mem::transmute::<ElimFn<T>, ElimFn<Erased>>(self) }
    }

    #[inline]
    pub const fn new(call: fn(T) -> Decomposition) -> Self {
        Self { call }
    }
}

impl<T> fmt::Debug for ElimFn<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(|ctor| ...)")
    }
}

impl ElimFn<Erased> {
    /// Interpret this type-erased generator as a generator for a specific type.
    /// # Safety
    /// You'd better be damn well sure that you're specifying the right type.
    #[inline]
    #[must_use]
    pub const unsafe fn unerase<T>(self) -> fn(T) -> Decomposition {
        // SAFETY: Same size, still a function pointer with the same arguments.
        unsafe { mem::transmute::<ElimFn<Erased>, ElimFn<T>>(self) }.call
    }
}

impl<T> Deref for ElimFn<T> {
    type Target = fn(T) -> Decomposition;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.call
    }
}

#[inline]
pub fn arbitrary<T: Construct>(prng: &mut WyRand, mut size: Size) -> Option<T> {
    let info = info::<T>();
    match info.type_former {
        PrecomputedTypeFormer::Algebraic(ref adt) => {
            let potential_loops = adt.potential_loops();
            let mut canary = 0_u8;
            loop {
                let (ctor, minus_one) = if size.should_recurse(prng)
                    && let Some(n) = NonZero::new(potential_loops.len())
                {
                    #[expect(
                        clippy::as_conversions,
                        clippy::cast_possible_truncation,
                        reason = "fine: definitely not > `u64::MAX` constructors"
                    )]
                    let i = prng.rand() as usize % n;
                    // SAFETY: Bounded by length above (see `% n`).
                    (unsafe { potential_loops.get_unchecked(i) }, true)
                } else {
                    let potential_leaves = adt.potential_leaves();
                    let n = NonZero::new(potential_leaves.len())?;
                    #[expect(
                        clippy::as_conversions,
                        clippy::cast_possible_truncation,
                        reason = "fine: definitely not > `u64::MAX` constructors"
                    )]
                    let i = prng.rand() as usize % n;
                    // SAFETY: Bounded by length above (see `% n`).
                    (unsafe { potential_leaves.get_unchecked(i) }, false)
                };
                let sizes = size._partition_into(ctor.n_big, prng, minus_one);
                let mut fields = (ctor.arbitrary_fields)(prng, sizes);
                // SAFETY: By the soundness of the type-`TypeId` relation,
                // which holds as long as no lifetime subtyping takes place,
                // and since only `'static` types have IDs and we can't generate functions,
                // it holds here.
                let result = unsafe { ctor.unerase::<T>() }(&mut fields);
                debug_assert!(
                    fields.is_empty(),
                    "internal `pbt` error: leftover terms after applying a constructor: {fields:#?}",
                );
                if let Some(result) = result {
                    return Some(result);
                }

                // If that failed, then there's (almost surely) a Sigma-type,
                // in which case its instantiability might be size-dependent
                // (e.g. a non-empty vector/string/etc.), in which case
                // we should occasionally bump the size just in case:
                let (next_canary, ovf) = canary.overflowing_add(1);
                canary = next_canary;
                if ovf {
                    let () = size._increment();
                }
            }
        }
        PrecomputedTypeFormer::Literal { generate, .. } => {
            // SAFETY: Undoing an earlier transmute.
            let generate = unsafe {
                mem::transmute::<fn(&mut WyRand) -> Erased, fn(&mut WyRand) -> T>(generate)
            };
            // All literals are instantiable.
            Some(generate(prng))
        }
    }
}

/// Check that eliminating a term and them
/// immediately constructing it again
/// is a no-op, i.e. the identity function.
/// # Panics
/// If that's not the case.
#[inline]
pub fn check_eta_expansion<T: Construct>() {
    let info = info::<T>();
    let PrecomputedTypeFormer::Algebraic(AlgebraicTypeFormer {
        ref all_constructors,
        eliminator,
        ..
    }) = info.type_former
    else {
        return;
    };
    // SAFETY: Undoing an earlier transmute.
    let eliminator = unsafe { mem::transmute::<ElimFn<Erased>, ElimFn<T>>(eliminator) };
    let () = search::assert_eq(32, |orig: &T| {
        let Decomposition {
            ctor_idx,
            mut fields,
        } = eliminator(orig.clone());
        // SAFETY: By the correct implementation of `eliminator`
        // (i.e., by macro logic plus the few implementations in this crate).
        #[expect(clippy::multiple_unsafe_ops_per_block, reason = "logically grouped")]
        let (ctor, _) = *unsafe { all_constructors.get_unchecked(ctor_idx.get().unchecked_sub(1)) };
        // SAFETY: By the soundness of the type-`TypeId` relation,
        // which holds as long as no lifetime subtyping takes place,
        // and since only `'static` types have IDs and we can't generate functions,
        // it holds here.
        let f = unsafe { ctor.unerase::<T>() };
        let constructed = f(&mut fields);
        assert!(
            fields.is_empty(),
            "internal `pbt` error: leftover terms after applying a constructor: {fields:#?}",
        );
        (constructed, Some(orig.clone()))
    });
}

#[inline]
pub fn visit_self<V: Construct, S: Construct>(s: &S) -> impl Iterator<Item = V> {
    visit_self_opt::<V, S>(s).cloned().into_iter()
}

#[inline]
pub fn visit_self_opt<V: Construct, S: Construct>(s: &S) -> Option<&V> {
    (type_of::<V>() == type_of::<S>()).then(|| {
        let s: *const S = ptr::from_ref(s);
        let s: *const V = s.cast();
        // SAFETY: `S` and `V` are the same type.
        unsafe { &*s }
    })
}

#[inline]
pub fn visit_self_owned<V: Construct, S: Construct>(s: S) -> Option<V> {
    (type_of::<V>() == type_of::<S>()).then(|| {
        let ptr: *const S = ptr::from_ref(&s);
        let ptr: *const V = ptr.cast();
        // SAFETY: `S` and `V` are the same type.
        let v: V = unsafe { ptr::read(ptr) };
        #[expect(clippy::mem_forget, reason = "intentional")]
        let () = mem::forget(s);
        v
    })
}

/// Deserialize a cached witness term of type `T` and push it into a typed term bucket.
#[inline]
pub(crate) fn deserialize_into_terms<T: Construct>(
    term: &cache::CachedTerm,
    terms: &mut TermsOfVariousTypes,
) -> bool {
    let Some(value) = cache::deserialize_term::<T>(term) else {
        return false;
    };
    terms.push(value);
    true
}
