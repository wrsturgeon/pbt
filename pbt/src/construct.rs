use {
    crate::{
        hash::{Map, SEED, Set},
        multiset::Multiset,
        reflection::{
            AlgebraicConstructors, Constructors, Erased, TermsOfVariousTypes, Type, TypeInfo,
            type_of,
        },
    },
    core::{
        fmt, mem,
        num::NonZero,
        ops::{Deref, DerefMut},
        ptr,
    },
    std::sync::Arc,
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

#[non_exhaustive]
#[repr(transparent)]
#[derive(Clone, Copy, Hash)]
pub struct ElimFn<T>(fn(T) -> Decomposition);

#[derive(/* NOT Copy */ Clone, Debug)]
pub struct Prng {
    /// The expected size of a term generated
    /// with this pseudorandom number generator.
    size: Option<NonZero<usize>>,
    /// The internal pseudorandom state
    /// used to generate arbitrary integers.
    state: WyRand,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Algebraic<T> {
    pub elimination_rule: ElimFn<T>,
    pub introduction_rules: Vec<IntroductionRule<T>>,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Literal<T> {
    pub generate: for<'prng> fn(&'prng mut WyRand) -> T,
    pub shrink: for<'orig> fn(&'orig T) -> Box<dyn Iterator<Item = T>>,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum TypeFormer<T> {
    Algebraic(Algebraic<T>),
    Literal(Literal<T>),
}

/// Decomposition of an algebraic value into its
/// constructor index and all immediate fields.
#[non_exhaustive]
#[derive(Debug)]
pub struct Decomposition {
    /// 1-indexed constructor/variant index.
    pub ctor_idx: NonZero<usize>,
    pub fields: TermsOfVariousTypes,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct IntroductionRule<T> {
    /// Function to invoke this constructor on a collection of fields.
    pub call: CtorFn<T>,
    /// The multiset of types necessary to call this constructor.
    pub immediate_dependencies: Multiset<Type>,
}

pub trait Construct: 'static + Clone {
    /// Generate arbitrary fields for a constructor chosen at runtime.
    fn arbitrary_fields_for_ctor(ctor_idx: NonZero<usize>, prng: &mut Prng) -> TermsOfVariousTypes;

    /// Cached type-level information from registration during initialization.
    /// It's always valid (and recommended) to copy and paste the following:
    /// ```
    /// # #[derive(Clone)]
    /// # struct NewType;
    /// # impl pbt::construct::Construct for NewType {
    /// # fn arbitrary_fields_for_ctor(ctor_idx: core::num::NonZero<usize>, prng: &mut pbt::construct::Prng) -> pbt::reflection::TermsOfVariousTypes { todo!() }
    /// #[inline]
    /// fn info() -> &'static pbt::reflection::TypeInfo {
    ///     static CACHE: std::sync::OnceLock<std::sync::Arc<pbt::reflection::TypeInfo>> =
    ///         std::sync::OnceLock::new();
    ///     CACHE.get_or_init(|| {
    ///         pbt::reflection::register::<Self>(
    ///             pbt::hash::empty_set(),
    ///             &mut *pbt::reflection::_registry_mut(),
    ///         )
    ///     })
    /// }
    /// # fn register_all_immediate_dependencies(_: &pbt::hash::Set<pbt::reflection::Type>, _: &mut pbt::hash::Map<pbt::reflection::Type, std::sync::Arc<pbt::reflection::TypeInfo>>) {}
    /// # fn type_former() -> pbt::construct::TypeFormer<Self> { todo!() }
    /// # fn visit_deep<V: pbt::construct::Construct>(&self) -> impl Iterator<Item = &V> { pbt::construct::visit_self(self) }
    /// # fn visit_shallow<V: pbt::construct::Construct>(&self) -> impl Iterator<Item = &V> { pbt::construct::visit_self(self) }
    /// # }
    /// ```
    fn info() -> &'static TypeInfo;

    /// Run depth-first search on the global type dependency graph.
    /// All this needs to do in practice is to
    /// add `::pbt::reflection::type_of::<Self>()` to `visited`
    /// then call `::pbt::reflection::register::<T>(visited)`
    /// for each `T` in the `immediate_dependencies` fields of `Self::_constructors()`.
    /// Induction and caching take care of the rest.
    fn register_all_immediate_dependencies(
        visited: &Set<Type>,
        registry: &mut Map<Type, Arc<TypeInfo>>,
    );

    /// The exhaustive disjoint set of methods
    /// to construct a term of this type.
    fn type_former() -> TypeFormer<Self>;

    /// Visit all terms of type `V` in this abstract syntax tree.
    /// Your implementation should always follow this formula:
    /// `pbt::construct::visit_self(self).chain(... recurse into fields ...)`.
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = &V>;

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
    pub const fn new(f: fn(T) -> Decomposition) -> Self {
        Self(f)
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
        unsafe { mem::transmute::<ElimFn<Erased>, ElimFn<T>>(self) }.0
    }
}

impl Prng {
    #[must_use]
    #[inline(always)]
    pub fn new(size: Option<NonZero<usize>>) -> Self {
        Self {
            size,
            state: WyRand::new(u64::from(SEED)),
        }
    }

    /// Whether to choose a potential leaf or loop constructor.
    #[must_use]
    #[inline]
    fn should_recurse(&mut self) -> bool {
        let Some(n) = self.size else {
            return false;
        };
        {
            #![expect(
                clippy::as_conversions,
                clippy::cast_possible_truncation,
                reason = "fine: definitely not > `u64::MAX` constructors"
            )]
            self.state.rand() as usize % n != 0
        }
    }

    #[must_use]
    #[inline(always)]
    pub const fn size(&self) -> Option<NonZero<usize>> {
        self.size
    }

    #[inline]
    pub const fn u64(&mut self) -> u64 {
        self.state.rand()
    }
}

impl Deref for Prng {
    type Target = WyRand;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for Prng {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

#[inline]
#[expect(
    clippy::missing_panics_doc,
    reason = "no user-facing panics; only internal errors"
)]
pub fn arbitrary<T: Construct>(prng: &mut Prng) -> Option<T> {
    let info = T::info();
    match info.constructors {
        Constructors::Algebraic(AlgebraicConstructors {
            ref potential_leaves,
            ref potential_loops,
            ..
        }) => {
            let ctor = if prng.should_recurse() {
                #[expect(
                    clippy::expect_used,
                    clippy::unwrap_in_result,
                    reason = "internal invariant"
                )]
                let n = NonZero::new(potential_loops.len())
                    .expect("internal `pbt` error: attempting to recurse on non-inductive type");
                #[expect(
                    clippy::as_conversions,
                    clippy::cast_possible_truncation,
                    reason = "fine: definitely not > `u64::MAX` constructors"
                )]
                let i = prng.rand() as usize % n;
                // SAFETY: Bounded by length above (see `% n`).
                unsafe { potential_loops.get_unchecked(i) }
            } else {
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
            let mut fields = T::arbitrary_fields_for_ctor(ctor.index, prng);
            // SAFETY: By the soundness of the type-`TypeId` relation,
            // which holds as long as no lifetime subtyping takes place,
            // and since only `'static` types have IDs and we can't generate functions,
            // it holds here.
            let f = unsafe { ctor.unerase::<T>() };
            let result = f(&mut fields);
            assert!(
                fields.is_empty(),
                "internal `pbt` error: leftover terms after applying a constructor",
            );
            Some(result)
        }
        Constructors::Literal { generate } => {
            // SAFETY: Undoing an earlier transmute.
            let generate = unsafe {
                mem::transmute::<fn(&mut WyRand) -> Erased, fn(&mut WyRand) -> T>(generate)
            };
            // All literals are instantiable.
            Some(generate(prng))
        }
    }
}

#[inline]
pub fn visit_self<V: Construct, S: Construct>(s: &S) -> impl Iterator<Item = &V> {
    visit_self_opt::<V, S>(s).into_iter()
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
