use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, IndexedCtorFn, IntroductionRule, Literal, TypeFormer,
        },
        hash::{Map, Set, empty_map, empty_set},
    },
    core::{
        any::{TypeId, type_name},
        fmt, mem,
        num::NonZero,
        ptr,
    },
    std::sync::{Arc, OnceLock, RwLock, RwLockWriteGuard},
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

/// A map from types to ordered collections of terms of those types.
/// This is used e.g. for constructors:
/// each constructor knows the multiset of types it needs to fill its fields,
/// so it can request exactly enough terms of various types to do so.
#[non_exhaustive]
#[repr(transparent)]
pub struct TermsOfVariousTypes {
    /// A map from types to ordered collections of terms of those types.
    map: Map<Type, (Vec<Erased>, fn(Vec<Erased>))>,
}

#[non_exhaustive]
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Type(TypeId);

/// The set of types that either *may* or *must*
/// be contained in any term of this type.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct TypeDependencies {
    /// 1-indexed constructor/variant index, if applicable.
    /// For dependencies of a type as a whole, this is `None`.
    pub ctor_idx: Option<NonZero<usize>>,
    /// The opaque Rust ID for this type.
    pub id: Type,
    /// The set of all types that *may* be contained in any term of this type.
    pub reachable: Set<Type>,
    /// The minimal bag of types that *must* be contained in any term of this type.
    /// If this is `None`, then this type has no constructors, i.e. is uninstantiable;
    /// note that this is a _very_ different state than `Some([empty])`!
    /// This field is not a multiset because, if this type is inductive,
    /// then the logic around how many times each type is unavoidable
    /// is too complex to be worth doing, especially since it provides no runtime benefit.
    pub unavoidable: Option<Set<Type>>,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct TypeInfo {
    pub constructors: Constructors,
    /// The union and intersection of the bag of types that
    /// may be contained in a value of this type.
    pub dependencies: TypeDependencies,
    /// The pretty-printed name of this type.
    pub name: &'static str,
    /// Whether this type is uninteresting: specifically, whether it is either
    /// non-inductive or a trivial wrapper around exactly one (other) type.
    /// Note that uninstantiable types *are* interesting, i.e. nontrivial.
    pub trivial: bool,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct AlgebraicConstructors {
    /// The exhaustive disjoint set of methods
    /// to construct a term of this type,
    /// each tagged with information about its type-level properties.
    pub all_tagged: Vec<(CtorFn<Erased>, TypeDependencies)>,
    /// All constructors for which `Self` is *unreachable*.
    /// Use this (when non-empty) to *force* generation of
    /// a *strictly smaller* value (in some sense).
    pub guaranteed_leaves: Vec<IndexedCtorFn<Erased>>,
    /// All constructors for which `Self` is *unavoidable*.
    /// Use this (when non-empty) to *force* generation of
    /// a *strictly larger* value (in some sense).
    pub guaranteed_loops: Vec<IndexedCtorFn<Erased>>,
    /// All constructors for which `Self` is *avoidable*.
    /// This is guaranteed to be non-empty because
    /// Rust disallows coinductive types (i.e. streams, infinite-size types, etc.)
    /// Use this (when non-empty) to *allow* generation of
    /// a smaller value (in some sense).
    pub potential_leaves: Vec<IndexedCtorFn<Erased>>,
    /// All constructors for which `Self` is *reachable*.
    /// Use this (when non-empty) to *allow* generation of
    /// a *larger* value (in some sense).
    pub potential_loops: Vec<IndexedCtorFn<Erased>>,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum Constructors {
    Algebraic(AlgebraicConstructors),
    Literal {
        generate: for<'prng> fn(&'prng mut WyRand) -> Erased,
    },
}

impl Constructors {
    #[inline]
    #[must_use]
    pub fn algebraic<T>(all_tagged: Vec<(CtorFn<T>, TypeDependencies)>) -> Self {
        Self::Algebraic(AlgebraicConstructors::new::<T>(all_tagged))
    }

    #[inline]
    #[must_use]
    pub fn literal<T>(generate: for<'prng> fn(&'prng mut WyRand) -> T) -> Self {
        Self::Literal {
            // SAFETY: Same size, still a function pointer with the same arguments.
            generate: unsafe {
                mem::transmute::<
                    for<'prng> fn(&'prng mut WyRand) -> T,
                    for<'prng> fn(&'prng mut WyRand) -> Erased,
                >(generate)
            },
        }
    }
}

impl AlgebraicConstructors {
    /// Partition a set of constructors into subsets
    /// that will be useful for generation and shrinking.
    /// # Panics
    /// If constructors are out of order (for bookkeeping)
    /// or if every constructor forces creation of
    /// another term of type `Self` (since generation would never halt).
    #[inline]
    #[must_use]
    pub fn new<T>(all_tagged: Vec<(CtorFn<T>, TypeDependencies)>) -> Self {
        // SAFETY: Same size, still a function pointer with the same arguments.
        let all_tagged = unsafe {
            mem::transmute::<
                Vec<(CtorFn<T>, TypeDependencies)>,
                Vec<(CtorFn<Erased>, TypeDependencies)>,
            >(all_tagged)
        };
        #[cfg(debug_assertions)]
        {
            let ctor_indices = all_tagged
                .iter()
                .map(|&(_, TypeDependencies { ctor_idx, .. })| ctor_idx);
            // SAFETY: Starts from one, monotonically increasing, ergo never zero
            let expected_indices =
                (1..=all_tagged.len()).map(|i| Some(unsafe { NonZero::new_unchecked(i) }));
            assert!(
                Iterator::eq(ctor_indices, expected_indices),
                "Constructor indices are out of order (should be 1, 2, ...): {all_tagged:#?}",
            );
        }
        let guaranteed_leaves: Vec<IndexedCtorFn<Erased>> = all_tagged
            .iter()
            .enumerate()
            .filter(|&(_, &(_, ref deps))| deps.is_guaranteed_leaf())
            .map(|(index, &(call, _))| {
                IndexedCtorFn {
                    call,
                    #[expect(
                        clippy::arithmetic_side_effects,
                        reason = "in-memory list length cannot exceed `usize`"
                    )]
                    // SAFETY: in-memory list length cannot exceed `usize`
                    index: unsafe { NonZero::new_unchecked(index + 1) },
                }
            })
            .collect();
        let guaranteed_loops: Vec<IndexedCtorFn<Erased>> = all_tagged
            .iter()
            .enumerate()
            .filter(|&(_, &(_, ref deps))| deps.is_guaranteed_loop())
            .map(|(index, &(call, _))| {
                IndexedCtorFn {
                    call,
                    #[expect(
                        clippy::arithmetic_side_effects,
                        reason = "in-memory list length cannot exceed `usize`"
                    )]
                    // SAFETY: in-memory list length cannot exceed `usize`
                    index: unsafe { NonZero::new_unchecked(index + 1) },
                }
            })
            .collect();
        let potential_leaves: Vec<IndexedCtorFn<Erased>> = all_tagged
            .iter()
            .enumerate()
            .filter(|&(_, &(_, ref deps))| deps.is_potential_leaf())
            .map(|(index, &(call, _))| {
                IndexedCtorFn {
                    call,
                    #[expect(
                        clippy::arithmetic_side_effects,
                        reason = "in-memory list length cannot exceed `usize`"
                    )]
                    // SAFETY: in-memory list length cannot exceed `usize`
                    index: unsafe { NonZero::new_unchecked(index + 1) },
                }
            })
            .collect();
        let potential_loops: Vec<IndexedCtorFn<Erased>> = all_tagged
            .iter()
            .enumerate()
            .filter(|&(_, &(_, ref deps))| deps.is_potential_loop())
            .map(|(index, &(call, _))| {
                IndexedCtorFn {
                    call,
                    #[expect(
                        clippy::arithmetic_side_effects,
                        reason = "in-memory list length cannot exceed `usize`"
                    )]
                    // SAFETY: in-memory list length cannot exceed `usize`
                    index: unsafe { NonZero::new_unchecked(index + 1) },
                }
            })
            .collect();
        debug_assert!(
            !potential_leaves.is_empty(),
            "internal `pbt` error: allegedly coinductive type: `{}`",
            type_name::<T>(),
        );
        Self {
            all_tagged,
            guaranteed_leaves,
            guaranteed_loops,
            potential_leaves,
            potential_loops,
        }
    }
}

impl TermsOfVariousTypes {
    #[inline]
    #[must_use]
    pub fn get<T: Construct>(&self) -> Option<&[T]> {
        let id = type_of::<T>();
        let &(ref v, _drop) = self.map.get(&id)?;
        let v: *const Vec<Erased> = ptr::from_ref(v);
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
        let &mut (ref mut v, _drop) = self.map.get_mut(&id)?;
        let v: *mut Vec<Erased> = ptr::from_mut(v);
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
                    !self.map.iter().any(|(_, &(ref v, _))| v.is_empty()),
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
        Self { map: empty_map() }
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
            let (v, drop) = self
                .map
                .remove(&type_of::<T>())
                .expect("internal `pbt` error: failed to remove empty vector of terms");
            drop(v)
        }
        opt
    }

    #[inline]
    pub fn push<T: Construct>(&mut self, t: T) {
        let id = type_of::<T>();
        let &mut (ref mut v, _drop) = self.map.entry(id).or_insert_with(|| {
            let v: Vec<T> = vec![];
            // SAFETY: Same collection type without elements;
            // creating a `Vec<T>` first to avoid size/alignment issues.
            let v = unsafe { mem::transmute::<Vec<T>, Vec<Erased>>(v) };
            let drop: fn(Vec<Erased>) = |v| {
                // SAFETY: Undoing the `transmute` above.
                drop(unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(v) })
            };
            (v, drop)
        });
        let v: *mut Vec<Erased> = ptr::from_mut(v);
        let v: *mut Vec<T> = v.cast();
        // SAFETY: Undoing the earlier `transmute` in `push` (the only entry point);
        // no operations are ever performed on the erased `Vec<Erased>` state.
        let v = unsafe { v.as_mut_unchecked() };
        v.push(t)
    }
}

impl Default for TermsOfVariousTypes {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TermsOfVariousTypes {
    #[inline]
    fn drop(&mut self) {
        for (_, (v, drop)) in self.map.drain() {
            drop(v)
        }
    }
}

impl fmt::Debug for TermsOfVariousTypes {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set()
            .entries(
                self.map
                    .iter()
                    .map(|(&k, &(ref v, _drop))| vec![k; v.len()]),
            )
            .finish()
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

impl TypeDependencies {
    /// Whether `Self` is *unreachable*.
    #[inline]
    #[must_use]
    pub fn is_guaranteed_leaf(&self) -> bool {
        !self.is_potential_loop()
    }

    /// Whether `Self` is *unavoidable*.
    #[inline]
    #[must_use]
    pub fn is_guaranteed_loop(&self) -> bool {
        self.unavoidable
            .as_ref()
            .is_some_and(|unavoidable| unavoidable.contains(&self.id))
    }

    /// Whether any term of this type contains `Self`,
    /// even transitively or indirectly via mutual induction.
    /// For example, a tree structure that contains `Box<Self>` is inductive,
    /// even though `Box` acts as a layer of indirection.
    /// Note that this library takes a functional view of e.g. lists as inductive,
    /// since any non-empty list can be seen as cons'ing an element onto another list.
    #[inline]
    #[must_use]
    pub fn is_inductive(&self) -> bool {
        self.reachable.contains(&self.id)
    }

    /// Whether a term of this type exists.
    /// Internally, this asks whether the set of constructors is non-empty,
    /// so this technically relies on the exhaustive nature of the set of constructors;
    /// i.e., garbage in (whem implementing the trait) means garbage out (here).
    #[inline]
    #[must_use]
    pub fn is_instantiable(&self) -> bool {
        self.unavoidable.is_some()
    }

    /// Whether `Self` is *avoidable*.
    #[inline]
    #[must_use]
    pub fn is_potential_leaf(&self) -> bool {
        !self.is_guaranteed_loop()
    }

    /// Whether `Self` is *reachable*.
    #[inline]
    #[must_use]
    pub fn is_potential_loop(&self) -> bool {
        self.reachable.contains(&self.id)
    }
}

impl fmt::Debug for Type {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match registry_locked().try_read() {
            Ok(registry) => match try_info_by_id(*self, &registry) {
                None => write!(f, "[unregistered type with ID {:?}]", self.0),
                Some(info) => f.write_str(info.name),
            },
            Err(locked) => write!(
                f,
                "[type with ID {:?} (registry is locked: {locked})]",
                self.0,
            ),
        }
    }
}

/// Lock the global type-information registry
/// and return a mutable reference to it.
/// **Do not use this unless you are a `pbt` maintainer.**
/// # Panics
/// If the lock has been poisoned.
#[inline]
pub fn _registry_mut<'lock>() -> RwLockWriteGuard<'lock, Map<Type, Arc<TypeInfo>>> {
    #[expect(clippy::expect_used, reason = "extremely unlikely")]
    registry_locked()
        .write()
        .expect("internal `pbt` error: type registry lock poisoned")
}

/// Register a type with the global registry of type dependency information.
/// If this function is called, then the function is *not* already in the registry,
/// and the return value of this function will be *automatically* added to the registry.
/// Do not attempt either operation manually from within this function.
#[inline]
#[expect(
    clippy::too_many_lines,
    reason = "TODO: split into a few encapsulated functions"
)]
fn compute_type_info<T: Construct>(
    mut visited: Set<Type>,
    registry: &mut Map<Type, Arc<TypeInfo>>,
) -> TypeInfo {
    let () = T::register_all_immediate_dependencies(&visited, registry);

    let self_id = type_of::<T>();
    let not_already_visited = visited.insert(self_id);
    assert!(
        not_already_visited,
        "internal `pbt` error: `visited` already contained `Self = {}` (`visited` was {visited:?})",
        type_name::<T>(),
    );

    let type_former = T::type_former();
    let shallow_ctors = match type_former {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules, ..
        }) => introduction_rules,
        TypeFormer::Literal(Literal { generate, .. }) => {
            return TypeInfo {
                constructors: Constructors::literal(generate),
                dependencies: TypeDependencies {
                    ctor_idx: None,
                    id: self_id,
                    reachable: empty_set(),
                    unavoidable: Some(empty_set()),
                },
                name: type_name::<T>(),
                trivial: true,
            };
        }
    };

    // Necessary to do this here, since we don't want *transitive* dependencies;
    // we care only whether this type wraps a single other type,
    // not anything about the type that's being wrapped or any transitive dependencies.
    let trivial = if let [
        IntroductionRule {
            ref immediate_dependencies,
            ..
        },
    ] = *shallow_ctors.as_slice()
    {
        let n_fields: usize = immediate_dependencies
            .iter()
            .filter_map(|(&id, count)| {
                // Count only inductive (i.e. interesting) types:
                (visited.contains(&id) || info_by_id(id, registry).dependencies.is_inductive())
                    .then_some(count.get())
            })
            .sum();
        n_fields <= 1
    } else {
        false
    };

    let mut constructors: Vec<(CtorFn<T>, TypeDependencies)> = vec![];
    let mut reachable: Set<Type> = empty_set();
    let mut unavoidable: Option<Set<Type>> = None;
    for (
        i,
        IntroductionRule {
            call,
            immediate_dependencies,
        },
    ) in shallow_ctors.into_iter().enumerate()
    {
        let mut deps = TypeDependencies {
            ctor_idx: Some(
                #[expect(clippy::expect_used, reason = "extremely unlikely")]
                ONE.checked_add(i)
                    .expect("internal `pbt` error: more than `usize::MAX` constructors"),
            ),
            id: self_id,
            reachable: empty_set(),
            unavoidable: Some(empty_set()),
        };
        #[expect(clippy::expect_used, reason = "extremely unlikely")]
        let ctor_unavoidable: &mut Set<Type> = deps
            .unavoidable
            .as_mut()
            .expect("internal `pbt` error: the pope is no longer catholic");
        for (id, _count) in immediate_dependencies {
            let _: bool = reachable.insert(id);
            let _: bool = ctor_unavoidable.insert(id);
            if !visited.contains(&id) {
                let info = info_by_id(id, registry);
                let () = reachable.extend(&info.dependencies.reachable);
                let () = ctor_unavoidable
                    .extend(info.dependencies.unavoidable.as_ref().into_iter().flatten());
            }
        }
        unavoidable = Some(unavoidable.map_or_else(
            || ctor_unavoidable.clone(),
            |mut unavoidable| {
                // Multiset::intersection(&unavoidable, &ctor_unavoidable)
                let () = unavoidable.retain(|id| ctor_unavoidable.contains(id));
                unavoidable
            },
        ));
        let () = constructors.push((call, deps));
    }

    TypeInfo {
        constructors: Constructors::algebraic(constructors),
        dependencies: TypeDependencies {
            ctor_idx: None,
            id: self_id,
            reachable,
            unavoidable,
        },
        name: type_name::<T>(),
        trivial,
    }
}

/// Register a type with the global registry of type dependency information.
#[inline]
#[expect(
    clippy::implicit_hasher,
    reason = "this is actually a great lint, but there should be exactly one generic parameter here"
)]
pub fn register<T: Construct>(
    visited: Set<Type>,
    registry: &mut Map<Type, Arc<TypeInfo>>,
) -> Arc<TypeInfo> {
    // `mut`, so TOCTOU is a non-issue
    let id = type_of::<T>();
    let info = if let Some(info) = registry.get(&id) {
        info
    } else {
        let info = Arc::new(compute_type_info::<T>(visited, registry));
        registry.entry(id).or_insert(info)
    };
    Arc::clone(info)
}

/// Get a handle to the global type-information registry without trying to lock it.
/// **Do not use this unless you are a `pbt` maintainer.**
/// # Panics
/// If the lock has been poisoned.
#[inline]
fn registry_locked() -> &'static RwLock<Map<Type, Arc<TypeInfo>>> {
    static REGISTRY: OnceLock<RwLock<Map<Type, Arc<TypeInfo>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(empty_map()))
}

/// Get type-level characteristics of a type by its unique but opaque type ID.
/// # Panics
/// If the type has not yet been registered with `pbt`.
#[inline]
#[must_use]
#[expect(
    clippy::implicit_hasher,
    reason = "consistency; see the comment on `register`"
)]
pub fn info_by_id(id: Type, registry: &Map<Type, Arc<TypeInfo>>) -> Arc<TypeInfo> {
    #[expect(clippy::expect_used, reason = "extremely unlikely")]
    try_info_by_id(id, registry).expect("internal `pbt` error: unregistered type")
}

/// Get type-level characteristics of a type by its unique but opaque type ID.
/// Returns `None` if the type has not yet been registered with `pbt`.
#[inline]
#[expect(
    clippy::implicit_hasher,
    reason = "consistency; see the comment on `register`"
)]
pub fn try_info_by_id(id: Type, registry: &Map<Type, Arc<TypeInfo>>) -> Option<Arc<TypeInfo>> {
    registry.get(&id).map(Arc::clone)
}

#[inline]
#[must_use]
pub fn type_of<T: Construct>() -> Type {
    Type(TypeId::of::<T>())
}
