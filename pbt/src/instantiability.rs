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

/// Per-type progress while computing instantiability.
enum Mask {
    /// Which algebraic constructors are currently known to be instantiable.
    Algebraic(Box<[bool]>),
    /// Whether this literal has at least one generator.
    Literal(bool),
}

impl Mask {
    /// Whether this type is currently known to be instantiable.
    #[inline]
    #[must_use]
    fn is_instantiable(&self) -> bool {
        match *self {
            Self::Algebraic(ref constructors) => constructors.iter().any(|&enabled| enabled),
            Self::Literal(enabled) => enabled,
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
    let mut masks: HashMap<TypeId, Mask> = domain
        .iter()
        .map(|&ty| {
            let mask = match *naive
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): unregistered type")
            {
                Constructors::Algebraic(ref constructors) => {
                    Mask::Algebraic(iter::repeat_n(false, constructors.len()).collect())
                }
                Constructors::Literal { ref generators, .. } => {
                    Mask::Literal(!generators.is_empty())
                }
            };
            (ty, mask)
        })
        .collect();

    'fixed_point: loop {
        let mut changed = false;
        let instantiable_types: HashSet<TypeId> = domain
            .iter()
            .copied()
            .filter(|ty| masks.get(ty).is_some_and(Mask::is_instantiable))
            .chain(
                cache
                    .iter()
                    .filter_map(|(&ty, constructors)| (!constructors.is_empty()).then_some(ty)),
            )
            .collect();

        for &ty in &domain {
            let Constructors::Algebraic(ref constructors) = *naive
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): unregistered type")
            else {
                continue;
            };
            let Mask::Algebraic(ref mut constructor_masks) = *masks
                .get_mut(&ty)
                .expect("INTERNAL ERROR (`pbt`): missing instantiability mask")
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

    for &ty in &domain {
        let constructors = match *naive
            .get(&ty)
            .expect("INTERNAL ERROR (`pbt`): unregistered type")
        {
            Constructors::Algebraic(ref constructors) => {
                let Mask::Algebraic(ref constructor_masks) = *masks
                    .get(&ty)
                    .expect("INTERNAL ERROR (`pbt`): missing instantiability mask")
                else {
                    unreachable!("INTERNAL ERROR (`pbt`): impossible instantiability mask")
                };
                let mut enabled: Vec<Constructor> = constructors
                    .iter()
                    .zip(constructor_masks)
                    .filter_map(|(constructor, &enabled)| enabled.then_some(constructor.clone()))
                    .collect();
                let () = enabled.sort_by_key(|constructor| constructor.field_types().total());
                Constructors::Algebraic(enabled.into())
            }
            Constructors::Literal { .. } => naive
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): unregistered type")
                .clone(),
        };
        let _old = cache.insert(ty, constructors);
    }
}

// TODO: generic tests
