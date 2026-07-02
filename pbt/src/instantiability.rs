//! Least-fixed-point filtering of unproductive variants.
//!
//! A constructor is productive iff every field type is productive; a type is productive iff any
//! constructor/generator for that type is productive. Starting with only literal generators enabled
//! and repeatedly enabling newly productive algebraic constructors yields the least fixed point,
//! excluding purely cyclic structures that cannot produce finite values.

use {
    crate::{
        hash::set,
        reflection::{Constructor, Constructors, Erased},
    },
    ahash::{HashMap, HashSet},
    alloc::collections::BTreeMap,
    core::{any::TypeId, iter},
};

/// Productive constructors for one type under the fixed-point mask.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
fn productive_constructors(
    ty: TypeId,
    naive: &BTreeMap<TypeId, Constructors<Erased>>,
    masks: &HashMap<TypeId, Box<[bool]>>,
) -> Constructors<Erased> {
    match *naive
        .get(&ty)
        .expect("INTERNAL ERROR (`pbt`): unregistered type")
    {
        Constructors::Algebraic(ref constructors) => {
            let constructor_masks = masks
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): missing instantiability mask");
            debug_assert_eq!(
                constructor_masks.len(),
                constructors.len(),
                "INTERNAL ERROR (`pbt`): mask size mismatch",
            );
            let mut enabled: Vec<Constructor> = constructors
                .iter()
                .zip(constructor_masks)
                .filter_map(|(constructor, &enabled)| enabled.then_some(constructor.clone()))
                .collect();
            let () = enabled.sort_by_key(|constructor| constructor.field_types().total());
            Constructors::Algebraic(enabled.into())
        }
        Constructors::Literal {
            deserialize,
            ref generators,
            serialize,
            shrink,
        } => {
            let generator_masks = masks
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): missing instantiability mask");
            debug_assert_eq!(
                generator_masks.len(),
                generators.len(),
                "INTERNAL ERROR (`pbt`): mask size mismatch",
            );
            Constructors::Literal {
                deserialize,
                generators: generators
                    .iter()
                    .zip(generator_masks)
                    .filter_map(|(&generator, &enabled)| enabled.then_some(generator))
                    .collect(),
                serialize,
                shrink,
            }
        }
    }
}

/// Collect all reachable types whose instantiability is not already cached.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
fn collect_uncached(
    ty: TypeId,
    naive: &BTreeMap<TypeId, Constructors<Erased>>,
    cache: &HashMap<TypeId, Constructors<Erased>>,
    domain: &mut HashSet<TypeId>,
) {
    if cache.contains_key(&ty) || !domain.insert(ty) {
        return;
    }

    let Constructors::Algebraic(ref constructors) = *naive
        .get(&ty)
        .expect("INTERNAL ERROR (`pbt`): unregistered type")
    else {
        return;
    };

    for constructor in &**constructors {
        for field in constructor.dedup_fields() {
            let () = collect_uncached(field, naive, cache, domain);
        }
    }
}

/// Cache all types reachable through productive constructors.
#[inline]
fn cache_productive_reachable(
    ty: TypeId,
    naive: &BTreeMap<TypeId, Constructors<Erased>>,
    masks: &HashMap<TypeId, Box<[bool]>>,
    cache: &mut HashMap<TypeId, Constructors<Erased>>,
) {
    if cache.contains_key(&ty) {
        return;
    }

    let constructors = productive_constructors(ty, naive, masks);
    let fields: Vec<TypeId> = constructors
        .algebraic()
        .iter()
        .flat_map(Constructor::dedup_fields)
        .collect();
    assert!(
        cache.insert(ty, constructors).is_none(),
        "INTERNAL ERROR (`pbt`): duplicate instantiability result",
    );

    for field in fields {
        let () = cache_productive_reachable(field, naive, masks, cache);
    }
}

/// Compute and cache the productive constructors reachable from `root`.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn update(
    root: TypeId,
    naive: &BTreeMap<TypeId, Constructors<Erased>>,
    cache: &mut HashMap<TypeId, Constructors<Erased>>,
) {
    if cache.contains_key(&root) {
        return;
    }

    let mut domain = set();
    let () = collect_uncached(root, naive, cache, &mut domain);
    let mut masks: HashMap<TypeId, Box<[bool]>> = domain
        .iter()
        .map(|&ty| {
            let constructors = match *naive
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): unregistered type")
            {
                Constructors::Algebraic(ref constructors) => {
                    iter::repeat_n(false, constructors.len()).collect()
                }
                Constructors::Literal { ref generators, .. } => {
                    iter::repeat_n(true, generators.len()).collect()
                }
            };
            (ty, constructors)
        })
        .collect();

    // TODO: `masks` is monotonic; how much can we move outside this loop,
    // and to what extent can we avoid re-collecting into `instantiable_types` inside the loop?

    let max_iterations: usize = masks
        .values()
        .map(|slice| slice.iter().map(|&b| usize::from(!b)).sum::<usize>())
        .sum();
    'fixed_point: for iteration in 0_usize.. {
        debug_assert!(
            iteration <= max_iterations,
            "non-terminating least-fixed-point loop",
        );

        let mut changed = false;

        // First, enumerate the types that were already instantiable
        // *before* we started analyzing `root`:
        let already_instantiable = cache
            .iter()
            .filter_map(|(&ty, constructors)| (!constructors.is_empty()).then_some(ty));

        // Then, enumerate all newly encountered types that aren't fully masked:
        let unmasked = domain.iter().copied().filter(|ty| {
            masks
                .get(ty)
                .is_some_and(|constructors| constructors.iter().any(|&enabled| enabled))
        });

        let instantiable_types: HashSet<TypeId> = unmasked.chain(already_instantiable).collect();

        #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
        for (&ty, constructor_masks) in &mut masks {
            let Constructors::Algebraic(ref constructors) = *naive
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): unregistered type")
            else {
                continue;
            };

            for (mask, constructor) in constructor_masks.iter_mut().zip(&**constructors) {
                if *mask {
                    continue;
                }
                if constructor
                    .dedup_fields()
                    .all(|field| instantiable_types.contains(&field))
                {
                    *mask = true;
                    changed = true;
                }
            }
        }

        if !changed {
            break 'fixed_point;
        }
    }

    let () = cache_productive_reachable(root, naive, &masks, cache);
}

#[cfg(test)]
mod tests {
    #[expect(
        dead_code,
        reason = "effectively just documentation for `update_*` tests"
    )]
    mod types {
        pub(super) enum Peano {
            O,
            S(Box<Self>),
        }

        pub(super) struct A(B);
        pub(super) struct B(C);
        pub(super) struct C;
    }

    use {
        super::*,
        crate::{hash::map, multiset::Multiset},
        alloc::sync::Arc,
        core::num::NonZero,
        pretty_assertions::assert_eq,
    };

    fn a_ctors() -> Constructors<Erased> {
        Constructors::Algebraic(Arc::new([Constructor {
            field_types: iter::once(TypeId::of::<types::B>()).collect(),
            index: const { NonZero::new(1).unwrap() },
        }]))
    }

    fn b_ctors() -> Constructors<Erased> {
        Constructors::Algebraic(Arc::new([Constructor {
            field_types: iter::once(TypeId::of::<types::C>()).collect(),
            index: const { NonZero::new(1).unwrap() },
        }]))
    }

    fn c_ctors() -> Constructors<Erased> {
        Constructors::Algebraic(Arc::new([Constructor {
            field_types: Multiset::new(),
            index: const { NonZero::new(1).unwrap() },
        }]))
    }

    #[test]
    fn update_with_cycle() {
        let peano = TypeId::of::<types::Peano>();
        let naive: BTreeMap<TypeId, Constructors<Erased>> = iter::once((
            peano,
            Constructors::Algebraic(Arc::new([
                Constructor {
                    field_types: Multiset::new(),
                    index: const { NonZero::new(1).unwrap() },
                },
                Constructor {
                    field_types: iter::once(peano).collect(),
                    index: const { NonZero::new(2).unwrap() },
                },
            ])),
        ))
        .collect();
        let mut cache = map();
        update(peano, &naive, &mut cache);
        let expected: HashMap<TypeId, Constructors<Erased>> = iter::once((
            peano,
            Constructors::Algebraic(Arc::new([
                Constructor {
                    field_types: Multiset::new(),
                    index: const { NonZero::new(1).unwrap() },
                },
                Constructor {
                    field_types: iter::once(peano).collect(),
                    index: const { NonZero::new(2).unwrap() },
                },
            ])),
        ))
        .collect();
        assert_eq!(cache, expected);
    }

    #[test]
    fn collect_uncached_transitive() {
        let a = TypeId::of::<types::A>();
        let b = TypeId::of::<types::B>();
        let c = TypeId::of::<types::C>();
        let naive: BTreeMap<TypeId, Constructors<Erased>> =
            [(a, a_ctors()), (b, b_ctors()), (c, c_ctors())]
                .into_iter()
                .collect();
        let cache = map();
        let mut domain = set();
        let () = collect_uncached(a, &naive, &cache, &mut domain);
        assert_eq!(cache, map());
        assert_eq!(domain, [a, b, c].into_iter().collect());
    }

    #[test]
    fn update_transitive() {
        let a = TypeId::of::<types::A>();
        let b = TypeId::of::<types::B>();
        let c = TypeId::of::<types::C>();
        let naive: BTreeMap<TypeId, Constructors<Erased>> =
            [(a, a_ctors()), (b, b_ctors()), (c, c_ctors())]
                .into_iter()
                .collect();
        let mut cache = map();
        update(a, &naive, &mut cache);
        let expected: HashMap<TypeId, Constructors<Erased>> =
            [(a, a_ctors()), (b, b_ctors()), (c, c_ctors())]
                .into_iter()
                .collect();
        assert_eq!(cache, expected);
    }

    #[test]
    fn update_transitive_partially_cached() {
        let a = TypeId::of::<types::A>();
        let b = TypeId::of::<types::B>();
        let c = TypeId::of::<types::C>();
        let naive: BTreeMap<TypeId, Constructors<Erased>> =
            [(a, a_ctors()), (b, b_ctors()), (c, c_ctors())]
                .into_iter()
                .collect();
        let mut cache: HashMap<TypeId, Constructors<Erased>> = iter::once((c, c_ctors())).collect();
        update(a, &naive, &mut cache);
        let expected: HashMap<TypeId, Constructors<Erased>> =
            [(a, a_ctors()), (b, b_ctors()), (c, c_ctors())]
                .into_iter()
                .collect();
        assert_eq!(cache, expected);
    }
}
