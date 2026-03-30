use {
    crate::{
        multiset::Multiset,
        reflection::{
            AlgebraicTypeFormer, Erased, PrecomputedTypeFormer, TermsOfVariousTypes, Type, info,
            type_of,
        },
        search,
        size::Size,
    },
    core::{fmt, mem, num::NonZero, ops::Deref, ptr},
    std::collections::BTreeSet,
    wyrand::WyRand,
};

#[non_exhaustive]
#[derive(Clone, Copy, Hash)]
pub struct CtorFn<T> {
    /// Function to invoke this constructor on a collection of fields.
    pub call: for<'terms> fn(&'terms mut TermsOfVariousTypes) -> T,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Hash)]
pub struct IndexedCtorFn<T> {
    /// Function to invoke this constructor on a collection of fields.
    pub call: CtorFn<T>,
    /// 1-indexed constructor/variant index.
    pub index: NonZero<usize>,
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
    pub generate: for<'prng> fn(&'prng mut WyRand) -> T,
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
    /// Function to invoke this constructor on a collection of fields.
    pub call: CtorFn<T>,
    /// The multiset of types necessary to call this constructor.
    pub immediate_dependencies: Multiset<Type>,
}

pub trait Construct: 'static + Clone + fmt::Debug + Eq {
    /// Generate arbitrary fields for a constructor chosen at runtime.
    fn arbitrary_fields_for_ctor(
        ctor_idx: NonZero<usize>,
        prng: &mut WyRand,
        size: Size,
    ) -> TermsOfVariousTypes;

    /// Run depth-first search on the global type dependency graph.
    /// All this needs to do in practice is to
    /// add `::pbt::reflection::type_of::<Self>()` to `visited`
    /// then call `::pbt::reflection::register::<T>(visited)`
    /// for each `T` in the `immediate_dependencies` fields of `Self::_constructors()`.
    /// Induction and caching take care of the rest.
    fn register_all_immediate_dependencies(visited: &BTreeSet<Type>);

    /// The exhaustive disjoint set of methods
    /// to construct a term of this type.
    fn type_former() -> TypeFormer<Self>;

    /// Visit all terms of type `V` in this abstract syntax tree.
    /// Your implementation should always follow this formula:
    /// `pbt::construct::visit_self(self).chain(... recurse into fields ...)`.
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V>;

    /// Visit all *non-nested* terms of type `V` in this abstract syntax tree.
    /// Your implementation should always follow this formula:
    /// `pbt::construct::visit_self_or(self, || ... recurse into fields ...)`.
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V>;
}

impl<T> CtorFn<T> {
    #[inline]
    #[must_use]
    pub const fn erase(self) -> CtorFn<Erased> {
        // SAFETY: Same size, still a function pointer with the same arguments.
        unsafe { mem::transmute::<CtorFn<T>, CtorFn<Erased>>(self) }
    }

    #[inline]
    pub const fn new(call: for<'terms> fn(&'terms mut TermsOfVariousTypes) -> T) -> Self {
        Self { call }
    }
}

impl CtorFn<Erased> {
    /// Interpret this type-erased generator as a generator for a specific type.
    /// # Safety
    /// You'd better be damn well sure that you're specifying the right type.
    #[inline]
    #[must_use]
    pub const unsafe fn unerase<T>(self) -> for<'terms> fn(&'terms mut TermsOfVariousTypes) -> T {
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
    type Target = for<'terms> fn(&'terms mut TermsOfVariousTypes) -> T;

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
pub fn arbitrary<T: Construct>(prng: &mut WyRand, size: Size) -> Option<T> {
    let info = info::<T>();
    match info.type_former {
        PrecomputedTypeFormer::Algebraic(ref adt) => {
            let potential_loops = adt.potential_loops();
            let ctor = if size.should_recurse(prng)
                && let Some(n) = NonZero::new(potential_loops.len())
            {
                #[expect(
                    clippy::as_conversions,
                    clippy::cast_possible_truncation,
                    reason = "fine: definitely not > `u64::MAX` constructors"
                )]
                let i = prng.rand() as usize % n;
                // SAFETY: Bounded by length above (see `% n`).
                unsafe { potential_loops.get_unchecked(i) }
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
                unsafe { potential_leaves.get_unchecked(i) }
            };
            let mut fields = T::arbitrary_fields_for_ctor(ctor.index, prng, size);
            // SAFETY: By the soundness of the type-`TypeId` relation,
            // which holds as long as no lifetime subtyping takes place,
            // and since only `'static` types have IDs and we can't generate functions,
            // it holds here.
            let f = unsafe { ctor.unerase::<T>() };
            let result = f(&mut fields);
            debug_assert!(
                fields.is_empty(),
                "internal `pbt` error: leftover terms after applying a constructor: {fields:#?}",
            );
            Some(result)
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
        #[expect(clippy::indexing_slicing, reason = "failing tests ought to panic")]
        let (ctor, _) = all_constructors[ctor_idx.get() - 1];
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
        (constructed, orig.clone())
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
pub fn visit_self_or<
    's,
    V: Construct,
    S: Construct,
    I: Iterator<Item = &'s V>,
    F: FnOnce() -> I,
>(
    s: &'s S,
    f: F,
) -> impl Iterator<Item = &'s V> {
    let opt = visit_self_opt::<V, S>(s);
    let recurse = opt.is_none();
    opt.into_iter().chain(recurse.then(f).into_iter().flatten())
}
