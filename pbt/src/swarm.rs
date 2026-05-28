//! A masked view into a type's constructors,
//! partitioned into potential leaves and loops.

use {
    crate::{
        pbt::Pbt,
        reflection::{self, Affordances, Erased},
        size::Size,
    },
    ahash::HashMap,
    alloc::collections::BTreeSet,
    core::{any::TypeId, mem, num::NonZero, ptr},
    wyrand::WyRand,
};

/// A masked view into a type's constructors,
/// partitioned into potential leaves and loops.
pub(crate) struct Swarm {
    /// A masked (partial) set of constructors for this type,
    /// partitioned into potential leaves and loops.
    masked: HashMap<TypeId, Affordances<Erased>>,
}

impl Swarm {
    /// A masked (partial) set of constructors for this type,
    /// partitioned into potential leaves and loops.
    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "For internal use only: invariant violations should fail loudly."
    )]
    pub fn affordances<T>(&mut self, prng: &mut WyRand) -> &Affordances<T>
    where
        T: Pbt,
    {
        let ty = TypeId::of::<T>();
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "OK: `u64` is already huge"
        )]
        let inserted: &Affordances<Erased> = self.masked.entry(ty).or_insert_with(|| {
            // TODO: should we expose global locks and
            // hold read guards in `self` to speed this up?
            // TODO: benchmark ^^

            // Look up the full affordances of this type,
            // so we can mask some to create a "swarm":
            let all_affordances = reflection::affordances::<T>();

            // If this type is uninstantiable, short-circuit:
            let Some(n_leaves) = NonZero::new(all_affordances.potential_leaves.len()) else {
                return Affordances {
                    potential_leaves: Box::new([]),
                    potential_loops: Box::new([]),
                };
            };

            // Produce a sorted list of indices of *instantiable variants only*:
            let ctor_indices: Vec<usize> = all_affordances
                .potential_leaves
                .iter()
                .map(|ctor| ctor.index)
                .chain(
                    all_affordances
                        .potential_loops
                        .iter()
                        .map(|ctor| ctor.index),
                )
                .collect::<BTreeSet<usize>>()
                .into_iter()
                .collect();
            let n_ctors = NonZero::new(ctor_indices.len())
                .expect("INTERNAL ERROR (`pbt`): nonzero leaves but zero constructors");

            // Uniformly select the *number* of features to enable
            // to avoid the near-50% collapse of binomial distributions.
            let mut select_n = prng.rand() as usize % n_ctors;
            // The above is on [0, n_ctors):
            // note that it can never select all features,
            // and it shouldn't ever select zero (uninstantiable without leaves).
            // We'll shift everything up one by mandatorily selecting a leaf.

            // If we selected more than half of the available constructors,
            // consider `false` to be active and "disable" the complement:
            let active = select_n <= (n_ctors.get() >> 1_u8);
            if !active {
                // SAFETY: By the modulo operation that set `select_n`.
                select_n = unsafe { n_ctors.get().unchecked_sub(select_n) };
                // The above is on (0, n_ctors]:
                // since we re-enable one leaf,
                // we should overshoot by one.
            }

            // Flip `select_n` features:
            let mut enabled: HashMap<usize, bool> =
                ctor_indices.iter().map(|&i| (i, false)).collect();
            let set_a_random_feature_to =
                |mut_enabled: &mut HashMap<usize, bool>, set: bool, mut_prng: &mut WyRand| -> () {
                    'rejection_sampling: loop {
                        let j = mut_prng.rand() as usize % n_ctors;
                        // SAFETY: modulo `n_ctors`, which equals `ctor_indices.len()` (set above)
                        let i = unsafe { ctor_indices.get_unchecked(j) };
                        let b_i = mut_enabled
                            .get_mut(i)
                            .expect("INTERNAL ERROR (`pbt`): witchcraft");
                        if *b_i != set {
                            *b_i = set;
                            break 'rejection_sampling;
                        }
                    }
                };
            for _ in 0..select_n {
                let () = set_a_random_feature_to(&mut enabled, true, prng);
            }

            // Then, additionally, enable a random leaf:
            let i_leaf = prng.rand() as usize % n_leaves;
            // SAFETY: modulo `n_leaves`, which equals `...leaves.len()` (set above)
            let i_ctor = unsafe { all_affordances.potential_leaves.get_unchecked(i_leaf) }.index;
            let leaf_enabled_mut = enabled
                .get_mut(&i_ctor)
                .expect("INTERNAL ERROR (`pbt`): invalid leaf index");
            if *leaf_enabled_mut == active {
                // Otherwise, enable a different feature at random
                // (note that this is not the "flip" loop above,
                // since it ensures the feature is `active`, not `true`):
                let () = set_a_random_feature_to(&mut enabled, active, prng);
            } else {
                *leaf_enabled_mut = active;
            }

            let typed = Affordances {
                potential_leaves: all_affordances
                    .potential_leaves
                    .iter()
                    .filter(|&ctor| {
                        *enabled
                            .get(&ctor.index)
                            .expect("INTERNAL ERROR (`pbt`): invalid leaf index")
                            == active
                    })
                    .cloned()
                    .collect(),
                potential_loops: all_affordances
                    .potential_loops
                    .iter()
                    .filter(|&ctor| {
                        *enabled
                            .get(&ctor.index)
                            .expect("INTERNAL ERROR (`pbt`): invalid loop index")
                            == active
                    })
                    .cloned()
                    .collect(),
            };

            // SAFETY: `T` is only ever the codomain of a function pointer.
            unsafe {
                mem::transmute::<
                    Affordances<T>, //
                    Affordances<Erased>,
                >(typed)
            }
        });
        // SAFETY: `T` is only ever the codomain of a function pointer.
        unsafe {
            ptr::from_ref::<Affordances<Erased>>(inserted)
                .cast::<Affordances<T>>()
                .as_ref_unchecked()
        }
    }
}

/// Generate an arbitrary term of some type.
#[inline]
#[expect(clippy::todo, reason = "TODO")]
pub(crate) fn arbitrary<T>(_swarm: &mut Swarm, _size: Size, _prng: &mut WyRand) -> T
where
    T: Pbt,
{
    todo!()
}
