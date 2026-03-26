use {
    crate::{
        construct::{Construct, Generate, GenerateErased, ShallowConstructor},
        hash::{Map, Set, empty_map, empty_set},
    },
    core::{
        any::{TypeId, type_name},
        fmt, mem,
        num::NonZero,
    },
    std::sync::{Arc, OnceLock, RwLock, RwLockWriteGuard},
};

/// One, as a non-zero integer. Stupid but efficient.
const ONE: NonZero<usize> = NonZero::new(1).unwrap();

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
pub struct Constructors {
    /// The exhaustive disjoint set of methods
    /// to construct a term of this type,
    /// each tagged with information about its type-level properties.
    pub all_tagged: Vec<(GenerateErased, TypeDependencies)>,
    /// All constructors for which `Self` is *unreachable*.
    /// Use this (when non-empty) to *force* generation of
    /// a *strictly smaller* value (in some sense).
    pub guaranteed_leaves: Vec<GenerateErased>,
    /// All constructors for which `Self` is *unavoidable*.
    /// Use this (when non-empty) to *force* generation of
    /// a *strictly larger* value (in some sense).
    pub guaranteed_loops: Vec<GenerateErased>,
    /// All constructors for which `Self` is *avoidable*.
    /// This is guaranteed to be non-empty because
    /// Rust disallows coinductive types (i.e. streams, infinite-size types, etc.)
    /// Use this (when non-empty) to *allow* generation of
    /// a smaller value (in some sense).
    pub potential_leaves: Vec<GenerateErased>,
    /// All constructors for which `Self` is *reachable*.
    /// Use this (when non-empty) to *allow* generation of
    /// a *larger* value (in some sense).
    pub potential_loops: Vec<GenerateErased>,
}

impl Constructors {
    /// Partition these constructors into sets that will be
    /// useful for generation and shrinking.
    /// # Panics
    /// If constructors are out of order (for bookkeeping)
    /// or if every constructor forces creation of
    /// another term of type `Self` (since generation would never halt).
    #[inline]
    #[must_use]
    pub fn new<T>(all_tagged: Vec<(Generate<T>, TypeDependencies)>) -> Self {
        // SAFETY: Same size, still a function pointer with the same arguments.
        let all_tagged = unsafe {
            mem::transmute::<
                Vec<(Generate<T>, TypeDependencies)>,
                Vec<(GenerateErased, TypeDependencies)>,
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
        let guaranteed_leaves: Vec<GenerateErased> = all_tagged
            .iter()
            .filter_map(|&(f, ref deps)| deps.is_guaranteed_leaf().then_some(f))
            .collect();
        let guaranteed_loops: Vec<GenerateErased> = all_tagged
            .iter()
            .filter_map(|&(f, ref deps)| deps.is_guaranteed_loop().then_some(f))
            .collect();
        let potential_leaves: Vec<GenerateErased> = all_tagged
            .iter()
            .filter_map(|&(f, ref deps)| deps.is_potential_leaf().then_some(f))
            .collect();
        let potential_loops: Vec<GenerateErased> = all_tagged
            .iter()
            .filter_map(|&(f, ref deps)| deps.is_potential_loop().then_some(f))
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

    let shallow_ctors = T::shallow_constructors();

    // Necessary to do this here, since we don't want *transitive* dependencies;
    // we care only whether this type wraps a single other type,
    // not anything about the type that's being wrapped or any transitive dependencies.
    let trivial = if let [
        ShallowConstructor {
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

    let mut constructors: Vec<(Generate<T>, TypeDependencies)> = vec![];
    let mut reachable: Set<Type> = empty_set();
    let mut unavoidable: Option<Set<Type>> = None;
    for (
        i,
        ShallowConstructor {
            construct,
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
        let () = constructors.push((construct, deps));
    }

    TypeInfo {
        constructors: Constructors::new(constructors),
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
