use {
    crate::{
        cache,
        multiset::Multiset,
        reflection::{
            AlgebraicTypeFormer, Erased, ErasedTermBuckets, PrecomputedTypeFormer, Type, info,
            type_of,
        },
        scc::StronglyConnectedComponents,
        search,
        size::{Size, Sizes},
    },
    core::{fmt, mem, num::NonZero, ops::Deref, ptr},
    std::collections::BTreeSet,
    wyrand::WyRand,
};

/// Wrapper around a constructor function that consumes erased field buckets.
#[non_exhaustive]
#[derive(Clone, Copy, Hash)]
pub struct CtorFn<T> {
    /// Function to pbt a term which is an
    /// application of this constructor to arbitrary fields.
    pub call: for<'terms> fn(&'terms mut ErasedTermBuckets) -> Option<T>,
}

/// A constructor function together with its stable constructor index and metadata.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Hash)]
pub struct IndexedCtorFn<T> {
    /// Generate precisely enough arbitrary fields
    /// to immediately invoke this constructor.
    pub arbitrary_fields:
        for<'prng> fn(&'prng mut WyRand, Sizes) -> Result<ErasedTermBuckets, MaybeUninstantiable>,
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
    /// Function that decomposes a value into constructor index and immediate fields.
    pub call: fn(T) -> Decomposition,
}

/// Algebraic type description: introductions plus one elimination rule.
#[derive(Clone, Debug)]
#[expect(clippy::exhaustive_structs, reason = "constructed in macros")]
pub struct Algebraic<T> {
    /// The rule that decomposes a value into constructor index and fields.
    pub elimination_rule: ElimFn<T>,
    /// The rules that construct values from immediate fields.
    pub introduction_rules: Vec<IntroductionRule<T>>,
}

/// Literal type description, used to bottom out structural recursion.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Literal<T> {
    /// Parse a cached string payload.
    pub deserialize: fn(&str) -> Option<T>,
    /// Generate a literal directly from the PRNG.
    pub generate: for<'prng> fn(&'prng mut WyRand) -> T,
    /// Convert a literal into a cache payload.
    pub serialize: fn(&T) -> String,
    /// Produce smaller literal candidates.
    pub shrink: fn(T) -> Box<dyn Iterator<Item = T>>,
}

/// The complete generation and shrinking description for one type.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum TypeFormer<T> {
    /// A type built from a finite set of constructors.
    Algebraic(Algebraic<T>),
    /// A type generated and shrunk directly.
    Literal(Literal<T>),
}

/// Failure modes for generation attempts.
#[derive(Clone, Debug)]
#[expect(clippy::exhaustive_enums, reason = "used internally")]
pub enum MaybeUninstantiable {
    /// The current size was insufficient; a larger size might work.
    Retry,
    /// The type has no available value.
    Uninstantiable,
}

/// Decomposition of an algebraic value into its
/// constructor index and all immediate fields.
#[derive(Debug)]
#[expect(clippy::exhaustive_structs, reason = "constructed in macros")]
pub struct Decomposition {
    /// 1-indexed constructor/variant index.
    pub ctor_idx: NonZero<usize>,
    /// The immediate fields grouped into erased buckets by concrete type.
    pub fields: ErasedTermBuckets,
}

/// One constructor rule for an algebraic type.
#[derive(Clone, Debug)]
#[expect(clippy::exhaustive_structs, reason = "constructed in macros")]
pub struct IntroductionRule<T> {
    /// Generate precisely enough arbitrary fields
    /// to immediately invoke this constructor.
    pub arbitrary_fields:
        for<'prng> fn(&'prng mut WyRand, Sizes) -> Result<ErasedTermBuckets, MaybeUninstantiable>,
    /// Function to invoke this constructor on a collection of fields.
    pub call: CtorFn<T>,
    /// The multiset of types necessary to call this constructor.
    pub immediate_dependencies: Multiset<Type>,
}

/// Types that can be generated, reflected, traversed, and shrunk by `pbt`.
pub trait Pbt: 'static + Clone + fmt::Debug + Eq {
    /// Register the immediate dependencies of `Self` within the current
    /// type-registration traversal.
    ///
    /// In practice, implementations should:
    /// compute `ty = ::pbt::reflection::type_of::<Self>()`,
    /// insert `ty` into `visited`,
    /// and then call `::pbt::reflection::register::<Dependency>(visited.clone(), sccs)`
    /// for each immediate dependency needed by `Self`.
    ///
    /// The surrounding registration walk is responsible for publishing the final
    /// type metadata and SCC node once this dependency recursion has completed.
    fn register_all_immediate_dependencies(
        visited: &mut BTreeSet<Type>,
        sccs: &mut StronglyConnectedComponents,
    );

    /// The exhaustive disjoint set of methods
    /// to pbt a term of this type.
    fn type_former() -> TypeFormer<Self>;

    /// Visit all terms of type `V` in this abstract syntax tree.
    /// Your implementation should always follow this formula:
    /// `pbt::pbt::visit_self(self).chain(... recurse into fields ...)`.
    fn visit_deep<V: Pbt>(&self) -> impl Iterator<Item = V>;
}

impl<T> CtorFn<T> {
    /// Erase this constructor function for storage in the global registry.
    #[inline]
    #[must_use]
    pub const fn erase(self) -> CtorFn<Erased> {
        // SAFETY: Same size, still a function pointer with the same arguments.
        unsafe { mem::transmute::<CtorFn<T>, CtorFn<Erased>>(self) }
    }

    /// Wrap a constructor function.
    #[inline]
    pub const fn new(call: for<'terms> fn(&'terms mut ErasedTermBuckets) -> Option<T>) -> Self {
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
    ) -> for<'terms> fn(&'terms mut ErasedTermBuckets) -> Option<T> {
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
    type Target = for<'terms> fn(&'terms mut ErasedTermBuckets) -> Option<T>;

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
    /// Erase this eliminator for storage in the global registry.
    #[inline]
    #[must_use]
    pub const fn erase(self) -> ElimFn<Erased> {
        // SAFETY: Same size, still a function pointer with the same arguments.
        unsafe { mem::transmute::<ElimFn<T>, ElimFn<Erased>>(self) }
    }

    /// Wrap an eliminator function.
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

/// Generate an arbitrary value of `T`, increasing size on retryable failures.
#[inline]
pub fn arbitrary<T: Pbt>(prng: &mut WyRand, mut size: Size) -> Option<T> {
    loop {
        match try_arbitrary::<T>(prng, size._copy()) {
            Ok(t) => return Some(t),
            Err(MaybeUninstantiable::Retry) => size._increment(),
            Err(MaybeUninstantiable::Uninstantiable) => return None,
        }
    }
}

#[inline]
/// Generate and push one constructor field.
/// # Errors
/// Returns [`MaybeUninstantiable::Retry`] or
/// [`MaybeUninstantiable::Uninstantiable`] from field generation after
/// draining unused field-size partitions for the abandoned constructor attempt.
pub fn push_arbitrary_field<T: Pbt>(
    fields: &mut ErasedTermBuckets,
    sizes: &mut Sizes,
    prng: &mut WyRand,
) -> Result<(), MaybeUninstantiable> {
    match sizes.try_arbitrary::<T>(prng) {
        Ok(t) => {
            fields.push(t);
            Ok(())
        }
        Err(error) => {
            sizes._discard_remaining();
            Err(error)
        }
    }
}

#[inline]
#[expect(
    clippy::needless_pass_by_value,
    reason = "`Size` is intentionally consumed as the total budget for one generation attempt"
)]
/// Try to generate an arbitrary term of type `T`.
/// # Errors
/// Returns [`MaybeUninstantiable::Retry`] when rejection sampling could not
/// decide at this size, or [`MaybeUninstantiable::Uninstantiable`] when `T`
/// has no structurally available constructor.
pub fn try_arbitrary<T: Pbt>(prng: &mut WyRand, size: Size) -> Result<T, MaybeUninstantiable> {
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
                    let Some(n) = NonZero::new(potential_leaves.len()) else {
                        return Err(MaybeUninstantiable::Uninstantiable);
                    };
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
                let mut fields = (ctor.arbitrary_fields)(prng, sizes)?;
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
                    return Ok(result);
                }

                // If that failed, then there's (almost surely) a Sigma-type,
                // in which case its instantiability might be size-dependent
                // (e.g. a non-empty vector/string/etc.), in which case
                // we should occasionally bump the size just in case:
                let Some(next_canary) = canary.checked_add(1) else {
                    return Err(MaybeUninstantiable::Retry);
                };
                canary = next_canary;
            }
        }
        PrecomputedTypeFormer::Literal { generate, .. } => {
            // SAFETY: Undoing an earlier transmute.
            let generate = unsafe {
                mem::transmute::<fn(&mut WyRand) -> Erased, fn(&mut WyRand) -> T>(generate)
            };
            // All literals are instantiable.
            Ok(generate(prng))
        }
    }
}

/// Check that eliminating a term and them
/// immediately constructing it again
/// is a no-op, i.e. the identity function.
/// # Panics
/// If that's not the case.
#[inline]
pub fn check_eta_expansion<T: Pbt>() {
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

/// Yield `s` as a `V` if `S` and `V` are the same registered type.
#[inline]
pub fn visit_self<V: Pbt, S: Pbt>(s: &S) -> impl Iterator<Item = V> {
    visit_self_opt::<V, S>(s).cloned().into_iter()
}

/// Borrow `s` as a `V` if `S` and `V` are the same registered type.
#[inline]
pub fn visit_self_opt<V: Pbt, S: Pbt>(s: &S) -> Option<&V> {
    (type_of::<V>() == type_of::<S>()).then(|| {
        let s: *const S = ptr::from_ref(s);
        let s: *const V = s.cast();
        // SAFETY: `S` and `V` are the same type.
        unsafe { &*s }
    })
}

/// Move `s` out as a `V` if `S` and `V` are the same registered type.
#[inline]
pub fn visit_self_owned<V: Pbt, S: Pbt>(s: S) -> Option<V> {
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
pub(crate) fn deserialize_cached_term_into_buckets<T: Pbt>(
    term: &cache::CachedTerm,
    terms: &mut ErasedTermBuckets,
) -> bool {
    let Some(value) = cache::deserialize_term::<T>(term) else {
        return false;
    };
    terms.push(value);
    true
}
