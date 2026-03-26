use {
    crate::{
        hash::{Map, Set},
        multiset::Multiset,
        reflection::{Type, TypeInfo, type_of},
    },
    core::{convert::Infallible, fmt, mem, num::NonZero, ptr},
    std::sync::Arc,
    wyrand::WyRand,
};

#[non_exhaustive]
#[repr(transparent)]
#[derive(Clone, Copy, Hash)]
pub struct Generate<T>(for<'prng> fn(&'prng mut Prng) -> T);

pub type GenerateErased = Generate<Infallible>;

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
pub struct ShallowConstructor<T> {
    pub construct: Generate<T>,
    pub immediate_dependencies: Multiset<Type>,
}

pub trait Construct: 'static + Clone {
    /// Cached type-level information from registration during initialization.
    /// It's always valid (and recommended) to copy and paste the following:
    /// ```
    /// # #[derive(Clone)]
    /// # struct NewType;
    /// # impl pbt::construct::Construct for NewType {
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
    /// # fn shallow_constructors() -> Vec<pbt::construct::ShallowConstructor<Self>> { todo!() }
    /// # fn visit<V: pbt::construct::Construct>(&self) -> impl Iterator<Item = &V> { pbt::construct::visit_self(self) }
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
    fn shallow_constructors() -> Vec<ShallowConstructor<Self>>;

    /// Visit all terms of type `V` in this abstract syntax tree.
    /// Your implementation should always follow this formula:
    /// `pbt::construct::visit_self(self).chain(... recurse into fields ...)`.
    fn visit<V: Construct>(&self) -> impl Iterator<Item = &V>;
}

impl<T> Generate<T> {
    #[inline]
    #[must_use]
    pub const fn erase(self) -> GenerateErased {
        // SAFETY: Same size, still a function pointer with the same arguments.
        unsafe { mem::transmute::<Generate<T>, GenerateErased>(self) }
    }

    #[inline]
    pub const fn new(f: for<'prng> fn(&'prng mut Prng) -> T) -> Self {
        Self(f)
    }
}

impl<T> fmt::Debug for Generate<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(|prng| ...)")
    }
}

impl GenerateErased {
    /// Interpret this type-erased generator as a generator for a specific type.
    /// # Safety
    /// You'd better be damn well sure that you're specifying the right type.
    #[inline]
    #[must_use]
    pub const unsafe fn unerase<T>(self) -> Generate<T> {
        // SAFETY: Same size, still a function pointer with the same arguments.
        unsafe { mem::transmute::<GenerateErased, Generate<T>>(self) }
    }
}

impl Prng {
    #[must_use]
    #[inline(always)]
    pub const fn new(size: Option<NonZero<usize>>, state: WyRand) -> Self {
        Self { size, state }
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

#[inline]
#[expect(clippy::todo, reason = "TODO")]
pub fn construct<T: Construct>(_prng: &mut Prng) -> T {
    todo!()
}

#[inline]
pub fn visit_self<V: Construct, S: Construct>(s: &S) -> impl Iterator<Item = &V> {
    (type_of::<V>() == type_of::<S>())
        .then(|| {
            let s: *const S = ptr::from_ref(s);
            let s: *const V = s.cast();
            // SAFETY: `S` and `V` are the same type.
            unsafe { &*s }
        })
        .into_iter()
}
